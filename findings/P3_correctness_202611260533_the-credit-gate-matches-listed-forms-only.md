---
id: PR431-CREDIT-GATE-LISTED-FORMS-ONLY
severity: P3
disposition: deferred
category: correctness
pr: 431
reviewed_sha: 9279c8370dd4c24d4c195ac2809fd785e401aa61
location: .github/scripts/validate-pr-commits.sh:283
provenance: introduced_by_feature
first_bad:
guard: the change that adds a form to the list
---

## Failure sequence

A pull request's own commit credits Claude using a spelling outside the gate's closed list.
The name or footer matcher does not recognize it, so the validator exits 0. This is a limit of
the list, not evidence that the commit contains no credit. The reviewed revision is
`9279c8370dd4c24d4c195ac2809fd785e401aa61`: its
[name matcher at line 283](https://github.com/sourcemaps/upstroke/blob/9279c8370dd4c24d4c195ac2809fd785e401aa61/.github/scripts/validate-pr-commits.sh#L283)
and [footer matcher at line 285](https://github.com/sourcemaps/upstroke/blob/9279c8370dd4c24d4c195ac2809fd785e401aa61/.github/scripts/validate-pr-commits.sh#L285)
remain unchanged under [ruling r72](https://github.com/sourcemaps/upstroke/pull/431#issuecomment-7183591095).

### Both passes' witnesses and original grades

These are the round-three reviewers' executed witnesses on that reviewed revision, not new
measurements made while filing this finding. The frontier report is archived as
`~/selfhost-noclaude-20261125/evidence/review-frontier-9279c83.md`; the independent regression
report is `~/selfhost-noclaude-20261125/evidence/review-regression-9279c83.comment.md`.

| Finding | Frontier witness and original grade | Regression witness and original grade |
|---|---|---|
| `PR431-FOOTER-PREFIX-CLASS` | **P2.** Real commits ending in `1️⃣ Generated with [Claude Code]`, both without and with a link, exited **0**. The robot-emoji link control exited **1**, `claude-generated`. | **P2.** `✨ Generated with Claude Code` changed from exit **1** at `1fd3777942b3d0c955c6bceccd9b045c00c5ad7b` to **0** at the reviewed revision. The published keycap footer still exited **0**. |
| `PR431-COAUTHOR-NAME-BOUNDARIES` | **P2.** With identical neutral mailboxes, `Claude-Opus` exited **0**, while `Claude Opus` exited **1**, `claude-co-author`. The hyphen stays inside the name word. | **P2.** `Claude Code`, with U+00A0 between the words and a neutral mailbox, changed from exit **1** at `1fd3777942b3d0c955c6bceccd9b045c00c5ad7b` to **0** at the reviewed revision. Git parsed the trailer in both runs. `Claude-Opus` still exited **0**. |

Both passes found that the earlier ledger's `fixed` labels overstated the repair. R72 retains
these witnesses and their original P2 grades; recording P3 here does not refute either pass.

### The closed list and its limits

R72 defines the gate's claim as refusing exactly these forms in the pull request's own commits:

1. An author or committer with email `noreply@anthropic.com` or name exactly `Claude`.
2. A `Co-Authored-By` line beginning with that key, or parsed by Git as that trailer, whose
   mailbox domain is `anthropic.com` or a subdomain, or whose display name contains `Claude`
   or `Anthropic` as a whole name word. The mailbox is the first `<…>` value or a bracket-less
   value ending in `@domain`, aside from a trailing parenthesised comment. Its domain follows
   its last `@`, which must be present; the local part is never parsed or searched. Without a
   mailbox, the value is a name only when Git parsed the trailer, and prose elsewhere.
3. A `Claude-Session:` line.
4. A whole line `Generated with Claude Code` or `Generated with [Claude Code]`, optionally
   with a final period and leading 🤖; the bracketed label may have a link.

Comparisons are case-insensitive, identity names are trimmed, and keys permit leading
whitespace and whitespace before the colon. The footer permits surrounding whitespace.
Name words keep ASCII letters, digits, apostrophes, hyphens and UTF-8 bytes together.

Everything else passes. The listed limits are:

- Any other footer prefix or wording, including the keycap and sparkle witnesses above.
- `Claude` joined to other text by a hyphen, apostrophe or non-ASCII character at another
  address, including `Claude-Opus` and the U+00A0 witness.
- A mailbox-less line Git does not parse as a trailer, such as `Co-Authored-By: Claude`
  followed by a blank line and later prose.
- Other keys and identities not listed. R72 notes `claude[bot]` as an unlisted author or
  committer name at another address; that example was read from the rule, not executed.

The listed false refusals are a person named Claude, any `anthropic.com` address in the
co-author rule, and a sentence Git parses as the co-author trailer. For example,
`Co-Authored-By: is the trailer the Claude Code adapter reads.` is refused when Git parses
it as the trailer. The same mailbox-less sentence outside that parsed block is prose under
the revised contract. A false refusal costs a reworded commit; the gate does not identify
whether the credited party is actually a model.

A direct run ignores replace refs and fails closed on shallow history and grafts; beyond
that, the claim is for the workflow's checkout. These are the r72 contract's bounds, not a
claim that the reviewed revision already implemented its separate graft and mailbox repairs.

### The r72 exception

`MAINTAINING.md` step 5 and `findings/PROCESS.md` §7 ordinarily require a witnessed finding
to be fixed, or rejected only with evidence that the witness is invalid. **R72 makes a
specific exception for its rows 1 and 2:** the footer and name witnesses still reproduce,
but the overbroad claim is corrected and the residual is filed together as this deferred P3.
Both passes originally graded both findings P2. R72's stated basis is that these cases need
spellings no tool writes, and that an explicit list can converge where a promise to recognize
all credit cannot. That is the ruling's rationale, not a claim of exhaustive tool coverage.

The exception does not defer the quoted-mailbox, unparsed-prose, graft, object-width or missing
`Anthropic` fixture findings: r72 assigns those separate repairs. Nor does it change the
general witness rule. The operator records the corresponding PR-body dispositions.

## What the change that takes this up should do

When a change adds a form to the list, name that form explicitly, update the contract and
matcher together, and add a fixture that distinguishes it from the existing rules and from
nearby human names or prose. Revisit the relevant limits and false refusals in this finding.
Until then, preserve the named passing-limit fixtures and the original review evidence;
neither a passing gate nor the corrected claim establishes that every Claude credit is refused.
