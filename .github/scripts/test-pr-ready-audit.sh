#!/usr/bin/env bash
# The pure parts of scripts/pr-ready-audit.sh and the whole of scripts/pr-review-parse.py,
# exercised against fixtures: the lane and its severity set, both review forms (the workflow's
# fenced JSON verdict and the frontier prose form) and the ledger, all four of which the one
# parser reads; the reader that takes its payload; the state a set of blockers puts a pull request
# in; the frontmatter id match and the newest-check-run choice.
# Everything that talks to GitHub is out of scope here, with one exception: the argument parser and
# the audit's own refusals are exercised by running the script as a process against a stub `gh`,
# because the defects they have had live in `main` and no call on the helpers can see them. git is
# in scope in one place, `finding_file_count`, which is run against a small repository this file
# builds: reading a list of names is where a shape rule stops helping and only a count will do. The stub answers every call
# with a marker and a failure, so an argument that should have been refused and instead reached
# GitHub shows up as a failed case rather than a live request. The audit's behaviour on a real
# pull request is still observed on the pull request.
#
# Needs `jq` as well as bash: the comment filter is a jq program, and running it is the only way
# to prove the trusted login reaches it as data rather than as program text.
#
# Each case names the defect it exists to catch, so a green run says what it proved:
#   MUT-LANE-LABEL-INPUT         the lane came from a label, not the branch prefix
#   MUT-LANE-TABLE-DRIFT         a prefix's lane, review effort or must-fix set disagreed with
#                                scripts/lane.sh's table -- asserted for all thirteen prefixes and
#                                not a sample, because the table this replaces WAS a sample: three
#                                `codex/` shapes and a catch-all, and every real prefix fell into
#                                the catch-all
#   MUT-LANE-PREFIX-UNKNOWN-DEFAULTS  a branch outside the vocabulary was given a lane instead of a
#                                refusal, which is how an unknown prefix was silently handed the
#                                most expensive review and the loosest fix set
#   MUT-LANE-LABEL-LIST-BY-HAND  the label list, or the label reconciliation, named lanes by hand
#                                instead of following the table, so it went stale the moment the
#                                table moved and left a retired `lane:*` label uncorrected
#   MUT-LANE-LABEL-SPLIT-ON-SPACES  the labels arrived as one space-joined field and every reader
#                                of it split on whitespace, which is not a label's boundary: a
#                                label named `lane:legacy docs` was reported and removed as
#                                `lane:legacy`, the removal of a label the pull request does not
#                                carry failed, and the run ended there with every pull request
#                                after it unaudited -- or a single label containing
#                                `ready-to-merge` was taken for the ready label, the same way
#   MUT-P3-EFFORT-FROM-LANE-ALONE  the P3 row's effort ignored the branch, or read the category as
#                                a PREFIX of the slug rather than as the whole first token -- so
#                                `bulk-fix-P3/docs-contract-cleanup` was reviewed at low effort --
#                                or answered low for a caller that passed no branch at all, which
#                                cheapens a review by omission
#   MUT-P3-COUNT-UNBOUNDED       the P3 lane had no tolerance: either `verdict-not-pass` still
#                                demanded a PASS no review could give, or the unwitnessed P3s were
#                                not counted against three, or a witnessed one was counted as
#                                unwitnessed
#   MUT-P3-LANE-DEFERS           the P3 lane let a P3 be deferred
#   MUT-JSON-SPLIT-BY-REGEX      findings read by splitting text on "},{" instead of a parser
#   MUT-NULL-WITNESS             a null witness field counted as a witness
#   MUT-MUST-UNSEEN              a MUST deviation was not flagged
#   MUT-BAD-SEVERITY-PASSES      a P9 finding passed as deferrable
#   MUT-STRAY-TOKEN-UNSEEN       a severity token outside the verdict object was ignored
#   MUT-VERDICT-FROM-PROSE       the verdict was read from prose, not the object with the findings
#   MUT-QUOTED-JSON-IS-JSON      a prose review quoting JSON was parsed as the JSON form
#   MUT-PROSE-HEADING-FINDING    a prose P1 written as a heading vanished
#   MUT-PROSE-LAST-VERDICT       the first VERDICT line won over the last
#   MUT-FRONTMATTER-BY-SUBSTRING an id in prose satisfied the frontmatter match
#   MUT-FRONTMATTER-PATTERN      an id with a dot matched as a regex
#   MUT-CHECK-RUN-FIRST-SUCCESS  an older success outranked a newer failure
#   MUT-CHECK-RUN-UNSTARTED      an unstarted run was ordered by a stand-in date, not its id
#   MUT-LEDGER-HEADER-AS-ROW     the header or separator line parsed as a row
#   MUT-REVIEWER-FROM-ORG        an organization owner was trusted as the reviewer, so the
#                                comment filter matched nothing and every pull request read
#                                no-review
#   MUT-REVIEWER-OVERRIDE-IGNORED  an explicit --reviewer was not preferred over the owner
#   MUT-REVIEWER-JQ-INJECTION    the trusted login was spliced into the comment filter's program
#                                text, so a value shaped like jq widened the author predicate to
#                                every commenter and another account's PASS became the verdict
#   MUT-REVIEWER-CASE-MISMATCH   the author comparison was case-sensitive, so naming the correct
#                                account in another spelling hid its blocking review as no-review
#   MUT-REVIEWER-ARG-EATEN       --reviewer read the next argument without checking it was one, so
#                                a following option was consumed as the login and switched off
#   MUT-REVIEWER-NULL-AUTHOR     the comment filter raised on a comment whose author is null, which
#                                failed the whole page it was on and dropped every review with it
#   MUT-REVIEW-LOOKUP-SUPPRESSED a failed comment lookup was reported as "no review", so the audit
#                                judged whatever survived the failure and an older PASS could win
#   MUT-TIMELINE-LOOKUP-SUPPRESSED  a failed timeline lookup was reported as "the base did not
#                                change", which is the check standing between a retarget and an
#                                enqueue
#   MUT-RULESET-LOOKUP-SUPPRESSED   a failed ruleset lookup was reported as "no ruleset requires an
#                                up-to-date branch", which drops the BEHIND blocker for the run
#   MUT-LOGIN-LOCALE-RANGE       the login check used a collating range, so what counted as an
#                                ASCII letter came from the locale and `evéntloops` was a login
#   MUT-REVIEWER-ENV-BEFORE-FLAG an inherited environment value was validated before the flag that
#                                replaces it, so a stale setting refused a run that never used it
#   MUT-REVIEWER-EMPTY-OVERRIDE  an empty --reviewer counted as no --reviewer, so an unset shell
#                                variable expanded into the flag moved trust to the repository owner
#   MUT-LIST-UNPAGINATED         a collection was read without --paginate, so page one was taken
#                                for the whole list and an active ruleset on page two was invisible
#   MUT-OPTION-LONE-HYPHEN       the option guard required a character after the hyphen, so a lone
#                                `-` was taken as a label name
#   MUT-REVIEW-PARSE-TRUNCATED   a parser that died partway printed a shorter findings list, and a
#                                shorter findings list is a weaker verdict, not an error
#   MUT-PROSE-READ-SUPPRESSED    a read that failed inside the parse was reported as "nothing
#                                matched", and the completeness marker was printed over it -- so a
#                                prose review carrying a P3 under a PASS parsed as a clean PASS
#                                with no findings. There is no read inside the parse for a status
#                                to be suppressed on now: the input is read once, and a read that
#                                failed is a parse that failed
#   MUT-META-FIELD-COLLAPSE      an empty META field let `read` with IFS=tab shift every column
#                                after it, so a review recording no commit read as one that did
#   MUT-PR-LOOKUP-SUPPRESSED     a failed pull-request read left empty fields to be audited
#   MUT-PR-LIST-SUPPRESSED       a failed open-pull-request listing read as "nothing is open"
#   MUT-FAIL-OPEN-SHAPE          a read whose failure cannot be seen: `for x in $(gh ...)`,
#                                `< <(gh ...)`, `|| true` on a gh or git command, or a gh or git
#                                command piped where the pipeline's status is read as an answer
#   MUT-FINDING-FILE-COUNT       the filed-finding count read a listing wrongly -- an error taken
#                                for "no files", or NUL-separated names put through `$(...)`,
#                                which drops NUL and leaves nothing to read
#   MUT-FINDING-FILE-READ-AS-MISS  a finding file the count could not read counted as a file that
#                                does not carry the id, so two files filing one id became one
#   MUT-FINDING-LEDGER-PREFIX-HISTORICAL  the count named only the ledger's CURRENT prefix while
#                                its caller passes a pull request's own head, so a head cut before
#                                the 2026-09-12 move counted 0 files for a finding filed once and
#                                the audit answered NOT-READY, `no-file:<id>`, at exit 0
#   MUT-REVIEW-FORGES-PROTOCOL   a review string carrying the parser's own separators wrote rows
#                                of the parser's language, END among them, and the audit read a
#                                finished parse from a marker instead of from a status
#   MUT-PARSE-WRITE-UNCHECKED    a protocol write returned EIO and the parse reported success, so
#                                one finding vanished from a findings list that still announced
#                                itself complete -- READY, and a merge call
#   MUT-PARSE-PAYLOAD-COUNT      the payload the audit reads did not declare how many records it
#                                carries, so a read that stopped part way was a shorter list
#   MUT-MANUAL-SCAN-FAILS-OPEN   the manual-blocker test was a command in a condition, which has
#                                two ways to be false: the answer, and a failure. `grep` exiting 2
#                                left state=READY with the blocker printed beside it
#   MUT-LEDGER-LOOKUP-SUPPRESSED a pull-request body the audit could not fetch was read as a body
#                                with no ledger rows in it, which is `no-row` for a filed finding
#   MUT-FRONTMATTER-UPSTREAM-ERROR-AS-MISS  the frontmatter read was a pipeline, so under
#                                `pipefail` the reader's failure (2) stood behind the matcher's
#                                ordinary "no match" (1) and the caller was handed an answer
#   MUT-PROSE-VERDICT-SUFFIX     the verdict check was anchored at one end, so it validated the
#                                TAIL of the token and read `VERDICT: FAIL<U+00C9>PASS` as PASS --
#                                and read it differently under `C`, in a file whose header
#                                forbids locale-dependent output
#   MUT-PROSE-HEAD-NOT-FIRST     the head check searched the whole listing instead of its first
#                                line, so a first marker it could not read was stepped over and a
#                                later line's commit came back as the reviewed head
#   MUT-FINDING-SYMLINK-AS-FILE  the candidate listing dropped the mode git records, so a
#                                `120000` symlink was read as a file and its TARGET STRING as
#                                that file's frontmatter
#   MUT-PROSE-VERDICT-SALVAGE    a verdict token that failed the whole-token check was handed to
#                                a fallback that stripped the leading colons and asterisks off
#                                it, so `VERDICT: ::PASS` -- rejected one line earlier -- came
#                                back out of the rejecting branch as `PASS`
#   MUT-FINDING-BLOB-OPEN-AS-MISS  the blob the matcher was to read could not be OPENED, so the
#                                redirection returned 1 with the matcher never run -- and 1 was
#                                the matcher's own word for "does not carry the id"
#   MUT-FINDING-LIST-SHORT-READ  the candidate listing stopped arriving part way through and the
#                                entries that had arrived were counted as all of them: `read`
#                                ends a loop at end of input and at a failed read alike
#   MUT-REVIEW-KIND-UNREADABLE-IS-PROSE  a format detection that could not read the file chose
#                                the prose parser, which reads a JSON review's `VERDICT:` line
#                                from outside its verdict object and misses every severity the
#                                object spells with a JSON escape. Detection is inside the one
#                                parser now, so a file it cannot read has no format at all
#   MUT-RETARGET-ANSWER-UNCHECKED  a helper's ANSWER is a write, and the write's failure was
#                                discarded: `{ echo yes; return 0; }` left the caller an empty
#                                string and a status of 0, `[[ "$x" == yes ]]` read that as "the
#                                base did not change", and a pull request retargeted since its
#                                review enqueued. The same case covers every channel that carries
#                                an answer back: the retarget `yes`/`no`, the ruleset's two flags,
#                                the comment id, and the lane's severity set -- each names its
#                                permissive answer and blocks on everything else
#   MUT-VERDICT-REVIVED-BY-TRUNCATION  the workflow form's verdict was the last block THAT PARSED
#                                rather than the last block, so a final object missing its closing
#                                fence or its final `}` was stepped over and the `PASS` quoted
#                                above it as an example became the verdict -- with the real
#                                object's `"P\u0031"` invisible to the stray scan
#   MUT-VERDICT-REVIVED-BY-INDENT  the verdict block was recognised by `^```json`, which is not
#                                CommonMark's rule: a fence may carry up to three leading spaces
#                                and its closing fence may be indented independently. One space
#                                before the real `CHANGES_REQUIRED` object and it stopped being a
#                                block at all, so the `PASS` quoted above it as an example became
#                                the verdict -- READY and a merge call, with `"P\u0031"` invisible
#                                to the stray scan. The case pins the recogniser AND the rule that
#                                stands when a recogniser is wrong again: the verdict is the
#                                comment's LAST block, and nothing that could be another block's
#                                material may follow it
#   MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN  the parse CHOSE between the places a verdict could be read
#                                from instead of refusing a comment that offers more than one.
#                                Three spellings of one fenced block, each making the WRONG block
#                                the one that was read: `{ "role_understanding"` with a space
#                                after the brace, which the tail check's enumeration of "block
#                                material" did not list; the real block inside a blockquote's
#                                `> `, which the recogniser did not see at all, so format
#                                detection fell back to prose and took the `VERDICT: PASS` written
#                                outside it; and a second block hidden inside an HTML comment,
#                                which a reader never sees and the scanner read as the last block.
#                                All three reached READY, exit 0 and one merge call out of a
#                                review carrying a P1 spelled `"P\u0031"`, which the stray scan
#                                cannot see either. A recogniser made cleverer closes the spelling
#                                in front of it; the count closes the class: EXACTLY ONE place a
#                                verdict may be read from, every fence run in the comment
#                                accounted for by the structure scan, nothing but whitespace after
#                                the block, no `VERDICT:` line outside it, and the form taken from
#                                the comment's marker rather than from whichever parser answers
#   MUT-BLOCK-CONTENT-NOT-MATERIAL  the CONTENT of a block the verdict was not read from was
#                                treated as inert by both the fence accounting and the candidate
#                                count, which looked only OUTSIDE the blocks the structure scan
#                                found. Accounting for every fence run says each one WAS consumed
#                                as a fence and nothing about WHICH block it went into: CommonMark
#                                suppresses a fence inside a raw-HTML block and this scan does not,
#                                so `<!--`, a ```text line and `-->` opened a block a reader never
#                                sees, it swallowed the real blocking verdict as its content, every
#                                run stayed accounted for, and a clean `PASS` appended after it was
#                                the only candidate left -- READY, exit 0, no blocker, out of a
#                                comment a CommonMark parse sees one CHANGES_REQUIRED verdict in.
#                                The same blind spot read from the other end left a `text`-fenced
#                                `role_understanding` object out of the count, so the count reached
#                                ZERO and zero was a road to the prose parser and its
#                                `VERDICT: PASS`. `<div>`, `<pre>` and any complete tag on a line
#                                of its own start HTML blocks too, so the class is the divergence
#                                and not the tag: a `json` fence -- at the start of its line or
#                                behind a blockquote's `>` or a list marker, which a rule anchored
#                                at the start of the line let through -- or a bare verdict opener
#                                inside a block the verdict was not read from is material
#   MUT-GUARD-READS-THE-WRITTEN-SPELLING  a guard compared THE CHARACTERS THE COMMENT IS STORED
#                                AS while the only consumer of what it guards compared what those
#                                characters DECODE TO -- two different documents, and the
#                                divergence is where something material lives. Two of them, one
#                                per decoder. CommonMark resolves character references in a fence
#                                info string, so ```` ```jso&#110; ```` opens the `language-json`
#                                block a reader sees and was no `json` fence to a rule reading the
#                                raw string: the blocking verdict hidden inside a swallowing
#                                block, invisible to the content rule, and a clean `PASS` the only
#                                candidate -- READY, exit 0, one merge call. `json.loads` resolves
#                                string escapes in a key, so `{"role_understandin\u0067"` opens
#                                object whose first key IS `role_understanding` to every reader of
#                                it and was no opener to the guard: the candidate count reached
#                                zero, and zero is the road to the prose parser and its
#                                `VERDICT: PASS`. Each guard now asks its question of what its own
#                                consumer reads -- the LANGUAGE a renderer gives the info string,
#                                by `markdown-it-py` 3.0.0's own function (escapes and references
#                                resolved and nothing else: it is not parsed as Markdown, so
#                                `js*on*` is `js*on*` there), and JSON's escapes for the key. A
#                                rule of its own for the info string was the same defect again:
#                                `jso&#110;` then a literal U+0085, and `jso&#00000110;`, whose
#                                eight digits both renderers resolve, each reached READY and a
#                                merge call past a parser that read the tag its own way
#   MUT-NAME-FOUND-IN-NO-RENDERING  the rule above, over-corrected: a tag no renderer names `json`
#                                read as `json`. `json&#133;x` and `json&#11;x` were resolved to
#                                U+0085 and U+000B and split there, and `markdown-it-py` 3.0.0
#                                resolves neither -- it leaves a reference to a control character
#                                written -- while cmark-gfm 0.29.0.gfm.6 resolves both and keeps the
#                                character inside the name. So an ordinary example block beside a
#                                review's one real verdict became a second candidate and a valid
#                                PASS was refused with "2 places a verdict could be read from":
#                                READY to NOT-READY, one merge call to none, out of a review that
#                                says nothing wrong and cannot be rewritten to clear it. A guard
#                                reads what its consumer reads in BOTH directions, and the
#                                spelling `json&amp;#133;x`, which renders byte-identically, is
#                                read the same way. The same code points written LITERALLY do end
#                                the word, because that renderer splits what it resolved with
#                                `str.split()`
#   MUT-STRAY-TOKEN-ENCODED      the stray severity scan ran over RAW TEXT while the severity only
#                                exists after JSON decoding, so `"severity":"P\u0031"` -- the
#                                spelling every witness in this family uses -- carried no `P1` for
#                                it to find, and the one check standing between an object outside
#                                the verdict block and READY found nothing. Both spellings are
#                                read now, and a doubled backslash is still not an escape
#   MUT-JSON-REPEATED-NAME-CHOSEN  the verdict object was decoded with unrestricted
#                                `json.loads`, which KEEPS THE LAST OCCURRENCE of a repeated
#                                name: a `findings` array carrying a P1 followed by a second
#                                `"findings":[]` deserialised to a clean PASS with no findings at
#                                all, the blocking array discarded before validation saw it and
#                                the severity spelled `P\u0031` so the stray scan could not see it
#                                either -- READY and a merge call out of a review that blocks. One
#                                level down the same shape erases a witness instead. The decode
#                                builds every object through a hook that refuses a repeated name,
#                                so the ambiguity is never a value, at any depth
#   MUT-PARSE-SHORT-WRITE        `write()` returned a smaller count than the payload and the count
#                                was discarded: under `PYTHONUNBUFFERED=1`, with `RLIMIT_FSIZE` at
#                                1,024 and a 3,560-byte result, `write(1, ..., 3560) = 1024` and
#                                the program exited 0 over a findings list 2,536 bytes short of
#                                itself, in both renderings
#   MUT-STAGING-PATH-SHARED      the staging pathname was derived from the destination, so every
#                                invocation publishing to one destination used one file: two of
#                                them overlapping renamed one's bytes under the other's exit 0
#   MUT-HERESTRING-FAILURE-AS-ANSWER  a value was fed to a command through `<<<`, which spills to
#                                a temporary file once it outgrows a pipe buffer: a file bash
#                                cannot create is a redirection that failed, the command never
#                                runs, and the shell returns 1 -- `grep`'s "no match". A compound
#                                command is skipped outright, where neither a captured status nor
#                                `set -e` can see it, and `read` or `mapfile` leaves what it was
#                                to fill holding whatever it held before. The guard is the class:
#                                the script carries no here-string and no here-document at all
set -euo pipefail
export PATH="/usr/bin:/bin:$PATH"
# NOTHING HERE WRITES INTO THE TREE IT JUDGES. This gate runs `scripts/pr-review-parse.py`
# as a script, which caches no bytecode, and hands its PATH to two inline programs as argv
# data -- and one of them, the decoder probe below, IMPORTS it, because asking the module's
# own decoders anything means loading the module; that probe sets the suppression on itself
# as well. The suppression is here too, on the environment every python this file starts
# inherits, because `scripts/` is an instrument path: a required gate that drops a build
# artifact there, CI included, is a thing meant to observe the code changing it instead, and
# the guarantee should not depend on every probe that imports remembering to ask for it.
# `.gitignore` carries the same rule, which it lacked.
export PYTHONDONTWRITEBYTECODE=1
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$script_dir/../.." && pwd)"
cd "$root"
failed=0
error() { echo "$*" >&2; failed=1; }
expect() {  # expect <case> <got> <want>
  [[ "$2" == "$3" ]] || error "$1: got [$2], want [$3]"
}
contains() {  # contains <case> <got> <want-substring>
  [[ "$2" == *"$3"* ]] || error "$1: got [$2], want it to contain [$3]"
}
command -v jq > /dev/null || { echo "test-pr-ready-audit: needs jq to run the comment filter" >&2; exit 1; }

PR_READY_AUDIT_LIBRARY=1 source scripts/pr-ready-audit.sh
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# --- the lane table: every prefix in the vocabulary, not a sample ------------------------------
# scripts/lane.sh is sourced by scripts/pr-ready-audit.sh, so sourcing the audit above defined it.
# EVERY PREFIX IS ASSERTED AND NOT A SAMPLE, because the table this replaces was a sample -- three
# `codex/` shapes and a catch-all -- and the catch-all was where the thirteen real prefixes went.
# The row is the assertion: prefix, lane, effort, must-fix, one line each, so a change to the table
# that does not change this file is a change nothing agreed to.
lane_rows=(
  'feature/a-slug|feature|max|P0 P1'
  'refactor/a-slug|refactor|max|P0 P1'
  'ci/a-slug|ci|max|P0 P1'
  'gate/a-slug|gate|max|P0 P1'
  'standards/a-slug|standards|high|P0 P1 P2'
  'docs/a-slug|docs|low|P0 P1'
  'findings/a-slug|findings|low|P0 P1'
  'fix-P0/correctness_a-slug|fix-p0p1|max|P0 P1'
  'fix-P1/correctness_a-slug|fix-p0p1|max|P0 P1'
  'fix-P2/correctness_a-slug|fix-p2|high|P0 P1 P2'
  'bulk-fix-P2/a-slug|bulk-fix-p2|max|P0 P1 P2'
  'fix-P3/correctness_a-slug|fix-p3|max|P0 P1 P2'
  'bulk-fix-P3/a-slug|fix-p3|max|P0 P1 P2'
)
for row in "${lane_rows[@]}"; do
  branch="${row%%|*}"; rest="${row#*|}"
  want_lane="${rest%%|*}"; rest="${rest#*|}"
  want_effort="${rest%%|*}"; want_fix="${rest#*|}"
  got_lane="$(lane_for "$branch")" \
    || { error "MUT-LANE-TABLE-DRIFT: [$branch] has no lane"; continue; }
  expect "MUT-LANE-TABLE-DRIFT lane [$branch]" "$got_lane" "$want_lane"
  expect "MUT-LANE-TABLE-DRIFT effort [$branch]" "$(effort_for "$got_lane" "$branch")" "$want_effort"
  expect "MUT-LANE-TABLE-DRIFT must-fix [$branch]" "$(must_fix_for "$got_lane")" "$want_fix"
done
# Eleven lanes, and `lane_list` is what the label list and both reconciliation loops are driven
# from: a lane in the table and not in the list is a lane whose label is never created.
expect MUT-LANE-LABEL-LIST-BY-HAND "$(lane_list | tr '\n' ' ')" \
  "feature refactor ci gate standards docs findings fix-p0p1 fix-p2 bulk-fix-p2 fix-p3 "
for row in "${lane_rows[@]}"; do
  branch="${row%%|*}"; rest="${row#*|}"; want_lane="${rest%%|*}"
  lane_list | grep -qxF "$want_lane" \
    || error "MUT-LANE-LABEL-LIST-BY-HAND: [$branch] is in lane [$want_lane], which lane_list omits"
done

# --- an unknown prefix is a refusal and never a default lane ------------------------------------
# THE WHOLE POINT OF THE TABLE. The catch-all it replaces handed a prefix nobody had thought about
# the most expensive review and the loosest fix set, in silence. The three `codex/` shapes are here
# because they are exactly what the old table read, and they are outside the vocabulary now.
for unknown in codex/findings-p3-7d2d8e9dc74a codex/findings-ba8fdc8dec2b codex/sweep-f3eb59a749fe \
               fix/sampler-kill-and-inspection feat/branch-gate-live-check bulk-fix-P0/a-slug \
               bulk-fix-P1/a-slug "" feature refactor FEATURE/a-slug "  "; do
  if lane_for "$unknown" > "$tmp/lane.out" 2>&1; then
    error "MUT-LANE-PREFIX-UNKNOWN-DEFAULTS: [$unknown] was given the lane [$(cat "$tmp/lane.out")]"
  fi
done
# and the refusal PRINTS THE VOCABULARY, because a name outside it is usually a name nobody has
# written down rather than a typo, and the answer to that is the table.
lane_for codex/findings-p3-7d2d8e9dc74a > "$tmp/refusal.out" 2>&1 || true
contains MUT-LANE-PREFIX-UNKNOWN-DEFAULTS "$(cat "$tmp/refusal.out")" "is not a known branch prefix"
for prefix in 'feature/' 'refactor/' 'ci/' 'gate/' 'standards/' 'docs/' 'findings/' 'fix-P0/' \
              'fix-P1/' 'fix-P2/' 'fix-P3/' 'bulk-fix-P2/' 'bulk-fix-P3/'; do
  contains "MUT-LANE-PREFIX-UNKNOWN-DEFAULTS vocabulary [$prefix]" "$(cat "$tmp/refusal.out")" "$prefix"
done

# --- the P3 row's effort is a function of the NAME, and it fails expensive ----------------------
# One row of the table makes effort a function of the branch rather than of the lane, and there are
# three ways to get it wrong.
#
# `effort_for fix-p3` with NO BRANCH must answer max: a caller that forgets the argument must not be
# cheapened into a low review by its own omission.
#
# The category is THE ONE THE NAME BEGINS WITH, one rule for both P3 prefixes: `fix-P3/` names carry
# it before the `_`, and a `bulk-fix-P3/` slug may begin with it -- `bulk-fix-P3/docs-contract-sweep`
# is a docs-contract batch and takes the docs-contract effort. Read as an exact `_`-delimited token
# instead, a hyphenated bulk slug can never equal a bare category name and every P3 batch is `max`,
# which is the table's own row not being applied.
#
# And the category ends at a WORD BOUNDARY. A bare byte prefix reads `docs-contract` out of
# `docs-contractual-review`, whose first word is `docs`, and hands the cheap review to a name nobody
# chose it for.
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3)" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 '')" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 fix-P3/docs-contract_a-slug)" low
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 fix-P3/docs-contract_docs-are-wrong)" low
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 bulk-fix-P3/docs-contract)" low
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 bulk-fix-P3/docs-contract-sweep)" low
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 fix-P3/correctness_a-slug)" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 fix-P3/liveness_a-slug)" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 bulk-fix-P3/correctness-sweep)" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 bulk-fix-P3/misc-cleanup)" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 bulk-fix-P3/a-batch-of-things)" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 bulk-fix-P3/docs-contractual-review)" max
expect MUT-P3-EFFORT-FROM-LANE-ALONE "$(effort_for fix-p3 bulk-fix-P3/docs)" max
# Every other lane is answered from the lane alone, so `effort_for "$lane"` stays true for them and
# a branch passed alongside changes nothing.
for row in "${lane_rows[@]}"; do
  branch="${row%%|*}"; rest="${row#*|}"
  want_lane="${rest%%|*}"; rest="${rest#*|}"; want_effort="${rest%%|*}"
  [[ "$want_lane" == fix-p3 ]] && continue
  expect "MUT-P3-EFFORT-FROM-LANE-ALONE lane-only [$want_lane]" "$(effort_for "$want_lane")" "$want_effort"
done

# --- a lane nobody can answer is answered by nobody ---------------------------------------------
# A lane this does not know is not a lane with nothing to fix in it: `[[ "  " == *" P1 "* ]]` is
# false, so an empty must-fix set makes every severity in every review deferrable. The permissive
# answer is said, never fallen into -- and the same holds for the effort, where the permissive
# answer is the cheap review.
for unknown in "" lane:feature findings-p3 findings-p1p2 feature-ish FEATURE fix-p0 fix-P2; do
  if must_fix_for "$unknown" > "$tmp/lane.out" 2>&1; then
    error "MUT-P3-LANE-DEFERS: lane [$unknown] was answered with [$(cat "$tmp/lane.out")]"
  fi
  if effort_for "$unknown" > "$tmp/lane.out" 2>&1; then
    error "MUT-P3-LANE-DEFERS: lane [$unknown] was given the effort [$(cat "$tmp/lane.out")]"
  fi
done

# --- whose review counts ------------------------------------------------------------------------
# A User owner stands in for the reviewer; that is the pre-organization behaviour and it stays.
expect MUT-REVIEWER-FROM-ORG "$(reviewer_login absent "" eventloops User)" eventloops
# An Organization owner authors no comments. Inheriting it yields a filter that matches nothing,
# which is not a stricter audit but a blind one, so it must fail rather than return a login.
if reviewer_login absent "" sourcemaps Organization > "$tmp/org.out" 2>&1; then
  error "MUT-REVIEWER-FROM-ORG: an Organization owner was accepted as the reviewer, got [$(cat "$tmp/org.out")]"
fi
expect MUT-REVIEWER-FROM-ORG "$(cat "$tmp/org.out")" ""
# An explicit override wins over either, and is the only way to audit an organization-owned repo.
expect MUT-REVIEWER-OVERRIDE-IGNORED "$(reviewer_login given eventloops sourcemaps Organization)" eventloops
expect MUT-REVIEWER-OVERRIDE-IGNORED "$(reviewer_login given someone-else eventloops User)" someone-else
# A missing login is not a reviewer either, whatever the type claims.
if reviewer_login absent "" "" User > "$tmp/empty.out" 2>&1; then
  error "MUT-REVIEWER-FROM-ORG: an empty owner login was accepted as the reviewer"
fi
# An override that was supplied and is empty is not an override that was not supplied. Reading the
# two as one is how `--reviewer ""` -- an unset shell variable expanded into the flag -- handed a
# User-owned repository's audit to its owner, over the reviewer the caller had named, and enqueued
# a pull request that reviewer had blocked. `given` is checked whatever it carries.
if reviewer_login given "" eventloops User > "$tmp/given-empty.out" 2>&1; then
  error "MUT-REVIEWER-EMPTY-OVERRIDE: an empty --reviewer fell back to the owner, got [$(cat "$tmp/given-empty.out")]"
fi
for bad_state in "" absent-ish 0 1 yes; do
  if reviewer_login "$bad_state" "" eventloops User > "$tmp/state.out" 2>&1; then
    error "MUT-REVIEWER-EMPTY-OVERRIDE: state [$bad_state] was treated as an answer, got [$(cat "$tmp/state.out")]"
  fi
done
# and `absent` still means absent: the owner stands in exactly where it did before.
expect MUT-REVIEWER-EMPTY-OVERRIDE "$(reviewer_login absent "" eventloops User)" eventloops
# The helper is the last gate before a string becomes the audit's notion of who may say PASS, and
# what it returns is put to a jq program, so it takes GitHub's login shape and nothing wider. Each
# of these is a typo, another option read by mistake, or an injection attempt; none is an account.
for bad in 'eventloops" or true or .user.login == "eventloops' '--enqueue' '-abc' 'abc-' 'a--b' \
           'github-actions[bot]' 'two words' 'a.b' 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'; do
  if reviewer_login given "$bad" eventloops User > "$tmp/bad.out" 2>&1; then
    error "MUT-REVIEWER-JQ-INJECTION: [$bad] was accepted as a login, got [$(cat "$tmp/bad.out")]"
  fi
done
# and nothing narrower: a real login, in any case, of any allowed length, is not refused.
for good in eventloops EventLoops a-b-c x 0 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; do
  expect MUT-REVIEWER-JQ-INJECTION "$(reviewer_login given "$good" sourcemaps Organization)" "$good"
done

# --- the login shape means the same thing in every locale ---------------------------------------
# A range inside a bash regex is resolved by the locale's collation: under en_US.utf8 `[A-Za-z0-9]`
# admits `é` and U+212A KELVIN SIGN, so `--reviewer evéntloops` passed the check that exists to
# stop it and reached the API as a login nobody has. Every fixture is run under every locale this
# machine offers and each must give the same answer under all of them. The non-ASCII fixtures are
# written as byte escapes rather than as literal characters, because what is under test is which
# bytes the check accepts, and a literal would depend on how this file is encoded and read.
locales=(C)
for loc in C.UTF-8 C.utf8 en_US.UTF-8 en_US.utf8; do
  locale -a 2>/dev/null | grep -qxF "$loc" && locales+=("$loc")
done
for loc in "${locales[@]}"; do
  for good in eventloops EventLoops a-b-c x 0 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; do
    LC_ALL="$loc" valid_login "$good" \
      || error "MUT-LOGIN-LOCALE-RANGE: [$good] is a login but LC_ALL=$loc refused it"
  done
  # ev<U+00E9>ntloops, <U+00E9>, <U+212A> alone and leading a word. None is a GitHub login; each is
  # inside `[A-Za-z0-9]` under a collation that is not C's.
  for bad in 'ev\xc3\xa9ntloops' '\xc3\xa9' '\xe2\x84\xaa' '\xe2\x84\xaavin' 'a\xc2\xa0b'; do
    if LC_ALL="$loc" valid_login "$(printf '%b' "$bad")"; then
      error "MUT-LOGIN-LOCALE-RANGE: [$bad] was accepted as a login under LC_ALL=$loc"
    fi
  done
done
# On a runner whose only locales are C and C.UTF-8 the defect does not reproduce -- under those two
# collations a range *is* ASCII -- so the fixtures above would pass against the unrepaired check
# there. This is the part of the guard that binds everywhere: the classes must be written out,
# because writing them as a range is what makes the answer the locale's to give.
if declare -f valid_login | grep -qE '\[[^]]*[A-Za-z0-9]-[A-Za-z0-9]'; then
  error "MUT-LOGIN-LOCALE-RANGE: valid_login matches with a collating range, whose meaning is the locale's, not ASCII"
fi

# --- the comment filter: the login is data, and spelling is not identity -------------------------
# The program the audit hands to `gh api --jq`, run here on a fixture by the jq on this machine.
# `gh` embeds its own jq, so this is a stand-in for that engine, not the engine itself; both read
# `env` and `ascii_downcase` the same way, and the audit's behaviour against the real API is
# observed on the pull request.
cat > "$tmp/comments.json" <<'EOF'
[{"created_at":"2026-09-01T00:00:00Z","id":1001,"user":{"login":"eventloops"},
  "body":"<!-- upstroke-frontier-review pr=1 head=x -->\nVERDICT: CHANGES_REQUIRED"},
 {"created_at":"2026-09-02T00:00:00Z","id":1002,"user":{"login":"a-contributor"},
  "body":"<!-- upstroke-frontier-review pr=1 head=x -->\nVERDICT: PASS"},
 {"created_at":"2026-09-03T00:00:00Z","id":1003,"user":{"login":"eventloops"},
  "body":"a comment carrying no review marker"}]
EOF
# Exported, not passed as a command prefix: a prefix is applied after the command's words are
# expanded, so a filter that went back to building the login into its text would read an empty
# variable here and look innocent. Exporting puts the value where both a splice and a lookup see it.
filter_as() {  # filter_as LOGIN: what the audit's comment filter selects from the fixture
  export UPSTROKE_AUDIT_REVIEWER="$1"
  jq -r "$(review_comment_filter)" "$tmp/comments.json"
}
expect MUT-REVIEWER-JQ-INJECTION "$(filter_as eventloops)" "2026-09-01T00:00:00Z 1001"
expect MUT-REVIEWER-JQ-INJECTION "$(filter_as a-contributor)" "2026-09-02T00:00:00Z 1002"
# Spliced into the program text this predicate is unconditionally true and the newest comment by
# anyone wins, which is another account's PASS. Compared as data it is a login nobody has.
expect MUT-REVIEWER-JQ-INJECTION \
  "$(filter_as 'eventloops" or true or .user.login == "eventloops')" ""
# GitHub resolves EventLoops and eventloops to one account, so the filter must too: the spelling
# is not the identity, and reading it as one turns a visible blocking review into `no-review`.
expect MUT-REVIEWER-CASE-MISMATCH "$(filter_as EventLoops)" "2026-09-01T00:00:00Z 1001"
expect MUT-REVIEWER-CASE-MISMATCH "$(filter_as EVENTLOOPS)" "2026-09-01T00:00:00Z 1001"
# Case-insensitivity widens the spelling of one account, never the set of accounts.
expect MUT-REVIEWER-CASE-MISMATCH "$(filter_as someone-else)" ""

# GitHub's REST schema lets an issue comment's `user` be null -- the account was deleted -- and a
# filter that lowers it raises. `--paginate` runs this program once per page, so raising on one
# unrelated comment loses every review on the page with it, and the newest review is exactly what
# a page can be holding. `body` is guarded on the same terms. Each of these is dropped as "not
# this reviewer's review"; nothing about the fetch's own health is inferred from them.
cat > "$tmp/nullable.json" <<'EOF'
[{"created_at":"2026-09-01T00:00:00Z","id":2001,"user":{"login":"eventloops"},
  "body":"<!-- upstroke-frontier-review pr=1 head=x -->\nVERDICT: CHANGES_REQUIRED"},
 {"created_at":"2026-09-02T00:00:00Z","id":2002,"user":null,
  "body":"<!-- upstroke-frontier-review pr=1 head=x -->\nVERDICT: PASS"},
 {"created_at":"2026-09-03T00:00:00Z","id":2003,"user":{"login":"eventloops"},"body":null},
 {"created_at":"2026-09-04T00:00:00Z","id":2004,"user":{},"body":"no login key at all"},
 {"created_at":"2026-09-05T00:00:00Z","id":2005,"user":{"login":null},"body":"a null login"}]
EOF
filter_nullable_as() {
  export UPSTROKE_AUDIT_REVIEWER="$1"
  jq -r "$(review_comment_filter)" "$tmp/nullable.json"
}
# The blocking review is still found, and jq does not raise: raising is what cost the page.
expect MUT-REVIEWER-NULL-AUTHOR "$(filter_nullable_as eventloops 2>&1)" "2026-09-01T00:00:00Z 2001"
if ! filter_nullable_as eventloops > /dev/null 2>&1; then
  error "MUT-REVIEWER-NULL-AUTHOR: the filter failed on a page holding a comment with a null author"
fi
# A deleted account is not an author to match against: an empty reviewer must not select it. No
# valid login is empty, which is the point -- the last defect here also came through a value that
# could not happen.
expect MUT-REVIEWER-NULL-AUTHOR "$(filter_nullable_as "" 2>&1)" ""
unset UPSTROKE_AUDIT_REVIEWER

# --- a lookup that failed is not a lookup that found nothing ------------------------------------
# Three helpers ask GitHub a question whose empty answer relaxes a blocker. Each used to give that
# empty answer when the request itself failed: the comment lookup by ending in `return 0`, the
# other two by `for x in $(gh api ...)`, which iterates zero times either way. A pull request whose
# reviewer had blocked it enqueued on an older PASS because of the first. The stub `gh` here fails
# every call, and each helper must report that rather than answer.
empty_path_early="$tmp/empty-path-early"
mkdir -p "$empty_path_early"
failing="$tmp/failing-gh"
mkdir -p "$failing"
printf '#!/usr/bin/env bash\necho "GH-FAILED $*" >&2\nexit 1\n' > "$failing/gh"
chmod +x "$failing/gh"
silent="$tmp/silent-gh"
mkdir -p "$silent"
printf '#!/usr/bin/env bash\nexit 0\n' > "$silent/gh"        # succeeds, finds nothing
chmod +x "$silent/gh"
timeline="$tmp/timeline-gh"
mkdir -p "$timeline"
printf '#!/usr/bin/env bash\necho 2026-09-05T00:00:00Z\n' > "$timeline/gh"
chmod +x "$timeline/gh"

if (PATH="$failing:$PATH"; repo=o/r; reviewer=eventloops; latest_review_id 999) > "$tmp/lr.out" 2>&1; then
  error "MUT-REVIEW-LOOKUP-SUPPRESSED: a failed comment lookup reported success, got [$(cat "$tmp/lr.out")]"
fi
# and the other direction, or a helper that always fails would pass the case above: a lookup that
# completed and found no review is the ordinary `no-review` answer, not a failure.
if ! got="$( (PATH="$silent:$PATH"; repo=o/r; reviewer=eventloops; latest_review_id 999) 2>&1 )"; then
  error "MUT-REVIEW-LOOKUP-SUPPRESSED: a completed lookup with no review reported failure"
fi
expect MUT-REVIEW-LOOKUP-SUPPRESSED "$got" ""

if (PATH="$failing:$PATH"; repo=o/r; base_changed_after 999 2026-09-01T00:00:00Z) > "$tmp/bc.out" 2>&1; then
  error "MUT-TIMELINE-LOOKUP-SUPPRESSED: an unreadable timeline was answered, got [$(cat "$tmp/bc.out")]"
fi
expect MUT-TIMELINE-LOOKUP-SUPPRESSED \
  "$( (PATH="$timeline:$PATH"; repo=o/r; base_changed_after 999 2026-09-01T00:00:00Z) )" yes
expect MUT-TIMELINE-LOOKUP-SUPPRESSED \
  "$( (PATH="$timeline:$PATH"; repo=o/r; base_changed_after 999 2026-09-09T00:00:00Z) )" no
expect MUT-TIMELINE-LOOKUP-SUPPRESSED \
  "$( (PATH="$silent:$PATH"; repo=o/r; base_changed_after 999 2026-09-01T00:00:00Z) )" no

# --- the answer itself is a write, and a write is a thing that can fail --------------------------
# THE LOOKUP SUCCEEDING IS NOT THE ANSWER ARRIVING. `{ echo yes; return 0; }` discarded the status
# of the one write that carries the blocking answer out of this helper: with that write failing,
# the helper exited 0 having said nothing, the caller's `[[ "$retargeted" == yes ]]` read the empty
# string as "the base did not change", and a pull request retargeted since its review was enqueued
# -- exit 0, one merge call. `/dev/full` is a real device whose every write returns ENOSPC, so this
# is the failure itself and not a stand-in for it.
full_status=0
( PATH="$timeline:$PATH"; repo=o/r; base_changed_after 999 2026-09-01T00:00:00Z > /dev/full ) 2>/dev/null \
  || full_status=$?
((full_status != 0)) \
  || error 'MUT-RETARGET-ANSWER-UNCHECKED: a [yes] whose write failed was reported as an answer'
# The other branch is held to the same rule, so the case above is about the write and not about
# which answer it was carrying.
full_status=0
( PATH="$timeline:$PATH"; repo=o/r; base_changed_after 999 2026-09-09T00:00:00Z > /dev/full ) 2>/dev/null \
  || full_status=$?
((full_status != 0)) \
  || error 'MUT-RETARGET-ANSWER-UNCHECKED: a [no] whose write failed was reported as an answer'

# AND AN ANSWER THAT ARRIVED IS ONE OF TWO WORDS. Everything else -- the empty string a partial or
# discarded write leaves behind most of all -- used to fall into `no`, which is the answer that
# lets a merge happen. The permissive answer has to be said. `retarget_blocker` sets a variable and
# runs no command, for the reason `audit_state` does, so these cases run it with an EMPTY PATH:
# a rewrite that reaches for an external command fails here whatever it reaches for.
blocker_of() {  # blocker_of ANSWER: the blocker that answer calls for, or nothing
  retarget_why="not-set"
  retarget_blocker "$1"
  printf '%s' "$retarget_why"
}
expect MUT-RETARGET-ANSWER-UNCHECKED "$(PATH="$empty_path_early" blocker_of no)" ""
expect MUT-RETARGET-ANSWER-UNCHECKED "$(PATH="$empty_path_early" blocker_of yes)" retargeted-after-review
for junk in "" " " YES Yes yes-ish "yes " " yes" no-ish "no " 0 1 true false; do
  expect "MUT-RETARGET-ANSWER-UNCHECKED [$junk]" "$(PATH="$empty_path_early" blocker_of "$junk")" \
    timeline-answer-unreadable
done

if (PATH="$failing:$PATH"; repo=o/r; ruleset_state) > "$tmp/rs.out" 2>&1; then
  error "MUT-RULESET-LOOKUP-SUPPRESSED: an unreadable ruleset list was answered, got [$(cat "$tmp/rs.out")]"
fi
expect MUT-RULESET-LOOKUP-SUPPRESSED "$( (PATH="$silent:$PATH"; repo=o/r; ruleset_state) )" "0 0"
# The same helper's answer is also a write, and its caller splits it by expansion: `${x%% *}` on
# anything but two flags and a space yields a `strict_up_to_date` that `((...))` reads as 0, which
# drops the BEHIND blocker for every pull request in the run.
ruleset_full_status=0
( PATH="$silent:$PATH"; repo=o/r; ruleset_state > /dev/full ) 2>/dev/null || ruleset_full_status=$?
((ruleset_full_status != 0)) \
  || error "MUT-RETARGET-ANSWER-UNCHECKED: a ruleset state whose write failed reported success"

# --- a list is not complete until the API says it is --------------------------------------------
# GitHub pages every collection and defaults this endpoint to 30. Thirty rulesets in any state hid
# an active strict one on page two: the audit read "nothing requires an up-to-date branch", a
# BEHIND pull request with a clean review went READY, and merge was called. A page-two HTTP 500
# gave the identical answer, because rows never asked for and rows that failed to arrive are the
# same missing rows. The stub records what was asked.
recording="$tmp/recording-gh"
mkdir -p "$recording"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >> "$GH_RECORD"\nexit 0\n' > "$recording/gh"
chmod +x "$recording/gh"
( export GH_RECORD="$tmp/asked.txt"; export PATH="$recording:$PATH"; repo=o/r; ruleset_state ) > /dev/null
contains MUT-LIST-UNPAGINATED "$(cat "$tmp/asked.txt")" "--paginate"

# The class, not the instance. Every `gh api` call in the script reads a collection unless it is
# one of the three that read a single object by id, and a collection read without --paginate is
# page one mistaken for the whole. This is the guard that catches the next endpoint rather than
# this one; the three exceptions are named so adding a fourth is a decision, not an omission.
join_continuations() {
  awk '{
    if (buf != "") { sub(/^[[:space:]]+/, ""); buf = buf " " $0 } else { buf = $0 }
    if (buf ~ /\\$/) { sub(/\\$/, "", buf); next }
    print buf; buf = ""
  } END { if (buf != "") print buf }'
}
# The audit AND the table it sources. Every shape rule below is about what this code may be
# written as, and scripts/lane.sh is part of the audit the moment it is sourced: a here-string
# added there spills the same way, and a `gh` or `git` command added there fails open the same way.
code_lines() { grep -vhE '^[[:space:]]*#' scripts/pr-ready-audit.sh scripts/lane.sh; }
while IFS= read -r call; do
  [[ "$call" == *"gh api "* ]] || continue
  [[ "$call" == *"--paginate"* ]] && continue
  case "$call" in
    *'gh api "repos/$repo" '*) continue ;;                              # the repository object
    *'gh api "repos/$repo/rulesets/$id" '*) continue ;;                 # one ruleset by id
    *'gh api "repos/$repo/issues/comments/$review_id" '*) continue ;;   # one comment by id
  esac
  error "MUT-LIST-UNPAGINATED: a collection is read without --paginate: [$call]"
done < <(code_lines | join_continuations)

# --- reads whose failure nothing can see --------------------------------------------------------
# Three shapes swallow the status of the command that produced the data, and each has cost this
# script a false READY: `for x in $(gh ...)` and `< <(gh ...)` iterate zero times whether the
# request failed or the answer was empty, and `|| true` turns any error into a successful empty
# read. The one survivor is the fetch, whose failure is checked immediately afterwards by asking
# git whether the objects arrived; it is named here so it stays the only one.
expect MUT-FAIL-OPEN-SHAPE "$(code_lines | grep -cE 'for [A-Za-z_]+ in \$\((gh|git)\b' || true)" 0
expect MUT-FAIL-OPEN-SHAPE "$(code_lines | grep -cE '< <\((gh|git)\b' || true)" 0
expect MUT-FAIL-OPEN-SHAPE \
  "$(code_lines | grep -E '\b(gh|git) .*\|\| true' | sed 's/^[[:space:]]*//' | tr '\n' ';')" \
  'git fetch -q origin "refs/pull/$pr/head" "$base" master 2>/dev/null || true;'
# A fourth shape, and the one that survived the first draft of this guard: a gh or git command
# piped into another and the pipeline's status read as an answer. `git diff ... | grep -q .` is
# false both when the diff is empty and when the diff failed, and the first means "edits no gate".
# Piping is fine where the result is captured and the status checked; it is not fine as a
# condition, because there the two outcomes are the same branch.
while IFS= read -r line; do
  [[ "$line" =~ (^|[^|])\|([^|]|$) ]] || continue
  [[ "$line" =~ (^|[[:space:]]|\()(gh|git)[[:space:]] ]] || continue
  [[ "$line" == *'="$('* ]] && continue
  error "MUT-FAIL-OPEN-SHAPE: a gh or git command is piped where its failure cannot be told from the pipeline's answer: [$line]"
done < <(code_lines | join_continuations)

# A fifth shape, one layer down: how a value is handed to a command. `<<<` is a here-document,
# and bash writes one to a TEMPORARY FILE once it outgrows a pipe buffer; a temporary file it
# cannot create is a redirection that failed, so the command never runs and the shell returns 1.
# For `grep` that 1 is "no match" -- an answer -- and the two become one. For a compound command
# it is worse still: `while ... done <<< "$x"` is skipped silently, with a status of 0, which
# neither a captured status nor `set -e` can see. Both shapes have been live in this file: one
# lost the only P1 in a 40,000-character review and printed END over it, the other would have
# dropped a findings list whole. Whole-line matching is done by expansion instead, and a read
# that needs a file is given a real one whose write is checked.
# The guard used to name the two shapes that had gone wrong -- `grep ... <<<` and `done <<<` --
# and each round found a third. `read ... <<<` leaves the variables it was to fill holding what
# they held before; `mapfile ... <<<` leaves the array empty, which for a list of pull requests is
# a run that audits nothing and exits 0; `$(wc -w <<< ...)` leaves an arithmetic test with nothing
# in it. So the class is named instead of its members: THIS SCRIPT CONTAINS NO HERE-STRING AND NO
# HERE-DOCUMENT AT ALL. Lines are walked by expansion, a fixed program text is written with
# `printf`, and a read that needs a file gets a real one whose write is checked.
expect MUT-HERESTRING-FAILURE-AS-ANSWER \
  "$(code_lines | grep -E '<<' | sed 's/^[[:space:]]*//' | tr '\n' ';')" ''

# --- the option parser, through main -------------------------------------------------------------
# `main` is what these exercise: the defect was in its argument loop and no call on a helper can
# reach it. The stub gh answers with a marker and a failure, so an argument that should have been
# refused and instead got as far as GitHub is a failed case here rather than a live request.
stub="$tmp/stub"
mkdir -p "$stub"
printf '#!/usr/bin/env bash\necho "GH-REACHED $*" >&2\nexit 97\n' > "$stub/gh"
chmod +x "$stub/gh"
run_audit() {  # run_audit ARG...: "<exit status>|<output, on one line>"
  local out status=0
  out="$(PATH="$stub:$PATH" bash scripts/pr-ready-audit.sh "$@" 2>&1)" || status=$?
  printf '%s|%s' "$status" "$(tr '\n' ' ' <<< "$out")"
}
# A bare --reviewer read $2 before asking whether there was one and died on `$2: unbound variable`.
got="$(run_audit --reviewer)"
contains MUT-REVIEWER-ARG-EATEN "$got" "2|refusing: --reviewer needs a login"
# The next option is not a login. Consuming it named a reviewer nobody has *and* switched off the
# flag it swallowed, so the run reported no-review on everything and enqueued nothing, exit 0.
got="$(run_audit --reviewer --enqueue 123)"
contains MUT-REVIEWER-ARG-EATEN "$got" "2|refusing: --reviewer needs a login, got the option [--enqueue]"
[[ "$got" == *GH-REACHED* ]] && error "MUT-REVIEWER-ARG-EATEN: --reviewer --enqueue reached GitHub"
# The short option is an option too. A guard written as `--*` let `-h` through, and
# `--ready-label -h` ran a whole audit under a label named for the help flag while `--ready-label
# --help` refused -- the same defect the body claimed fixed, surviving in the half of the option
# space the guard did not name. Every option this parser takes, and one it does not, in both
# positions: what is refused is the leading hyphen, not a list that can drift again.
for opt in -h --help --enqueue --apply --reviewer --ready-label -x; do
  got="$(run_audit --ready-label "$opt" --reviewer eventloops 123)"
  contains MUT-REVIEWER-ARG-EATEN "$got" "2|refusing: --ready-label needs a label name, got the option [$opt]"
  [[ "$got" == *GH-REACHED* ]] && error "MUT-REVIEWER-ARG-EATEN: --ready-label $opt reached GitHub"
  got="$(run_audit --reviewer "$opt" 123)"
  contains MUT-REVIEWER-ARG-EATEN "$got" "2|refusing: --reviewer needs a login, got the option [$opt]"
  [[ "$got" == *GH-REACHED* ]] && error "MUT-REVIEWER-ARG-EATEN: --reviewer $opt reached GitHub"
done
# A malformed login is refused at the flag, before any request: the filter's encoding is the
# second bar, not the only one.
got="$(run_audit --reviewer 'eventloops" or true or .user.login == "eventloops' --enqueue 123)"
contains MUT-REVIEWER-JQ-INJECTION "$got" "2|refusing: --reviewer ["
[[ "$got" == *GH-REACHED* ]] && error "MUT-REVIEWER-JQ-INJECTION: a malformed --reviewer reached GitHub"
# UPSTROKE_REVIEW_AUTHOR takes the same value by another road and gets the same check.
got="$(UPSTROKE_REVIEW_AUTHOR='eventloops" or true' run_audit 123)"
contains MUT-REVIEWER-JQ-INJECTION "$got" "2|refusing: UPSTROKE_REVIEW_AUTHOR="
[[ "$got" == *GH-REACHED* ]] && error "MUT-REVIEWER-JQ-INJECTION: a malformed UPSTROKE_REVIEW_AUTHOR reached GitHub"
# The guard must stop the malformed and only the malformed: a real login gets through to the work.
got="$(run_audit --reviewer eventloops 123)"
contains MUT-REVIEWER-ARG-EATEN "$got" "GH-REACHED"
# --ready-label reads its argument the same way and had the same two defects.
contains MUT-REVIEWER-ARG-EATEN "$(run_audit --ready-label)" "2|refusing: --ready-label needs a label name"
contains MUT-REVIEWER-ARG-EATEN "$(run_audit --ready-label --enqueue 123)" "got the option [--enqueue]"
# A label name is still a label name, and a lane:* one is still refused for being a lane label.
contains MUT-REVIEWER-ARG-EATEN "$(run_audit --ready-label queue-me --reviewer eventloops 123)" "GH-REACHED"
contains MUT-REVIEWER-ARG-EATEN "$(run_audit --ready-label lane:feature 123)" "must not be a lane:* label"

# Which source supplies the login is settled before the value is checked. Validating the
# environment first made an inherited value this run does not use a precondition for the flag that
# replaces it: `--reviewer eventloops` refused from a shell whose UPSTROKE_REVIEW_AUTHOR was
# stale, and so did --help, which reads nothing at all.
got="$(UPSTROKE_REVIEW_AUTHOR='bad_login' run_audit --reviewer eventloops 123)"
contains MUT-REVIEWER-ENV-BEFORE-FLAG "$got" "GH-REACHED"
[[ "$got" == *refusing:* ]] && error "MUT-REVIEWER-ENV-BEFORE-FLAG: an overridden UPSTROKE_REVIEW_AUTHOR still refused the run"
got="$(UPSTROKE_REVIEW_AUTHOR='bad_login' run_audit --help)"
contains MUT-REVIEWER-ENV-BEFORE-FLAG "$got" "0|"
[[ "$got" == *refusing:* ]] && error "MUT-REVIEWER-ENV-BEFORE-FLAG: --help refused because of a value it never reads"
# Precedence is not permission: the environment value is still checked when it is the one that
# will be trusted, and the message says which source named it.
got="$(UPSTROKE_REVIEW_AUTHOR='bad_login' run_audit 123)"
contains MUT-REVIEWER-ENV-BEFORE-FLAG "$got" "2|refusing: UPSTROKE_REVIEW_AUTHOR=[bad_login] is not a GitHub login."
[[ "$got" == *GH-REACHED* ]] && error "MUT-REVIEWER-ENV-BEFORE-FLAG: an invalid UPSTROKE_REVIEW_AUTHOR reached GitHub"
got="$(run_audit --reviewer 'bad_login' 123)"
contains MUT-REVIEWER-ENV-BEFORE-FLAG "$got" "2|refusing: --reviewer [bad_login] is not a GitHub login."

# --- a failed review lookup, through main -------------------------------------------------------
# The helper case above proves latest_review_id reports a failure. This proves the audit acts on
# it: `no-review` and "the lookup did not complete" are different lines in the table, and only the
# second says the audit does not know what the reviewer said. Reading the second as the first is
# what let a pull request its reviewer had blocked reach the merge queue on an older PASS.
# This stub answers just enough to reach the review lookup; with no review id the audit resolves
# no reviewed commit, so nothing here touches git or the network.
lookup="$tmp/lookup-gh"
mkdir -p "$lookup"
cat > "$lookup/gh" <<'GH'
#!/usr/bin/env bash
head=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
case "$*" in
  "repo view"*)               echo eventloops/upstroke ;;
  *"--jq .owner.login")       echo eventloops ;;
  *"--jq .owner.type")        echo User ;;
  *rulesets*)                 ;;                                 # no branch ruleset
  *check-runs*)               printf 'upstroke-ci\t10\tsuccess\nupstroke-pr-policy\t11\tsuccess\n' ;;
  *"/pulls?state=open"*)      exit "${STUB_PULLS_STATUS:-0}" ;;
  *timeline*)                 ;;                                 # no base change
  *"/comments?per_page=100"*) [[ -n "${STUB_REVIEW_BODY:-}" ]] && echo "2026-09-01T00:00:00Z ${STUB_REVIEW_ID:-5001}"
                              exit "${STUB_COMMENTS_STATUS:-1}" ;;
  *"/issues/comments/5001 --jq .created_at") echo 2026-09-01T00:00:00Z ;;
  *"/issues/comments/5001 --jq .body")       cat "$STUB_REVIEW_BODY" ;;
  *"--json body"*)            (( ${STUB_BODY_STATUS:-0} )) && exit "$STUB_BODY_STATUS"
                              echo "no ledger" ;;
  "pr view"*)                 (( ${STUB_PRVIEW_STATUS:-0} )) && exit "$STUB_PRVIEW_STATUS"
                              printf '%s\n%s\nfalse\nCLEAN\nmaster\n%s\n' \
                                "${STUB_BRANCH:-feature/x}" "$head" "$head"
                              # THE LABELS ARE LAST AND TAKE A LINE EACH, which is what the audit's
                              # own `--jq` asks gh for: a label name may hold a SPACE, so a
                              # space-joined field has no boundaries to read. STUB_LABELS is one
                              # label per line. The count gh declares ahead of them is derived from
                              # the same text, so this stub cannot disagree with itself;
                              # STUB_LABEL_COUNT overrides it, to model a read that came back split.
                              stub_labels_n=0
                              if [[ -n "${STUB_LABELS:-}" ]]; then
                                stub_labels_n="$(printf '%s\n' "$STUB_LABELS" | wc -l)"
                              fi
                              printf '%s\n' "${STUB_LABEL_COUNT:-$stub_labels_n}"
                              if [[ -n "${STUB_LABELS:-}" ]]; then printf '%s\n' "$STUB_LABELS"; fi ;;
  *) echo "GH-UNSTUBBED $*" >&2; exit 97 ;;
esac
GH
chmod +x "$lookup/gh"
run_lookup() {  # run_lookup: the audit's table line, with the comment fetch exiting $1
  STUB_COMMENTS_STATUS="$1" PATH="$lookup:$PATH" bash scripts/pr-ready-audit.sh 999 2>&1 | tr '\n' ' '
}
got="$(run_lookup 1)"
contains MUT-REVIEW-LOOKUP-SUPPRESSED "$got" "NOT-READY"
contains MUT-REVIEW-LOOKUP-SUPPRESSED "$got" "blockers=review-lookup-failed"
[[ "$got" == *no-review* ]] && error "MUT-REVIEW-LOOKUP-SUPPRESSED: a failed lookup was reported as no-review"
[[ "$got" == *GH-UNSTUBBED* ]] && error "MUT-REVIEW-LOOKUP-SUPPRESSED: the audit made a call this case does not model: [$got]"
# The same run with the fetch succeeding and finding nothing is the ordinary no-review line, so
# the case above cannot be passed by a script that reports a lookup failure for everything.
got="$(run_lookup 0)"
contains MUT-REVIEW-LOOKUP-SUPPRESSED "$got" "blockers=no-review"
[[ "$got" == *review-lookup-failed* ]] && error "MUT-REVIEW-LOOKUP-SUPPRESSED: a completed lookup was reported as failed"

# The third answer that channel can carry. A comment id is a number, and the two API paths built
# from it are the last thing it passes through; a value that is neither empty nor a number is a
# lookup that came back with something nobody checked, and it is not a review to go and fetch.
printf '```json\n{"verdict":"PASS","findings":[]}\n```\n' > "$tmp/id-check-review.md"
for bad_id in "not-a-number" "-5001" "5001;x" "P1" "0x10" "5001.0"; do
  got="$(STUB_REVIEW_BODY="$tmp/id-check-review.md" STUB_REVIEW_ID="$bad_id" run_lookup 0)"
  contains "MUT-RETARGET-ANSWER-UNCHECKED id [$bad_id]" "$got" "review-id-unreadable"
  contains "MUT-RETARGET-ANSWER-UNCHECKED id [$bad_id]" "$got" "NOT-READY"
  [[ "$got" == *GH-UNSTUBBED* ]] \
    && error "MUT-RETARGET-ANSWER-UNCHECKED: the audit fetched a comment id it could not read: [$got]"
done
# and a number still reaches the fetch, so the case above is about the shape of the id.
got="$(STUB_REVIEW_BODY="$tmp/id-check-review.md" run_lookup 0)"
[[ "$got" == *review-id-unreadable* ]] \
  && error "MUT-RETARGET-ANSWER-UNCHECKED: a numeric comment id was refused"

run_stub() {  # run_stub ARG...: "<exit status>|<output, on one line>", against the stub above
  local out status=0
  out="$(PATH="$lookup:$PATH" bash scripts/pr-ready-audit.sh "$@" 2>&1)" || status=$?
  printf '%s|%s' "$status" "$(tr '\n' ' ' <<< "$out")"
}
# A pull request whose own metadata could not be read is reported unaudited, not audited on the
# empty fields the failed read left behind. Read through `< <(...)` the status was invisible: the
# fields came back empty and the run died further down on an empty head, with no line for this
# pull request and no reason given.
got="$(STUB_PRVIEW_STATUS=1 run_stub 999)"
contains MUT-PR-LOOKUP-SUPPRESSED "$got" "blockers=pr-lookup-failed"
contains MUT-PR-LOOKUP-SUPPRESSED "$got" "NOT-READY"
[[ "$got" == *GH-UNSTUBBED* ]] && error "MUT-PR-LOOKUP-SUPPRESSED: the audit kept calling after the read failed: [$got]"
# and a readable pull request still reaches the review lookup, so the case above is about the
# failure and not about the stub.
got="$(run_stub 999)"
[[ "$got" == *pr-lookup-failed* ]] && error "MUT-PR-LOOKUP-SUPPRESSED: a readable pull request was reported unreadable"

# A ruleset state that is not two flags is not a ruleset state. `ruleset_state` can only write the
# four, so this drives `main` with the helper replaced -- which is what a partial write, or a
# rewrite of that helper, would leave it holding. `${rulesets%% *}` on anything else yields a
# `strict_up_to_date` that `((...))` reads as 0, and that drops the BEHIND blocker for every pull
# request in the run, so the value is checked whole before it is split.
ruleset_answer_run() {  # ruleset_answer_run ANSWER: "<status>|<output>" with ruleset_state saying ANSWER
  local out status=0
  out="$(
    RULESET_ANSWER="$1"
    ruleset_state() { printf '%s\n' "$RULESET_ANSWER"; }
    PATH="$lookup:$PATH" main --reviewer eventloops 999 2>&1
  )" || status=$?
  printf '%s|%s' "$status" "$(printf '%s' "$out" | tr '\n' ' ')"
}
for bad_rulesets in "" "0" "1" "0 0 0" "yes no" "0  0" " 0 0"; do
  got="$(ruleset_answer_run "$bad_rulesets")"
  contains "MUT-RETARGET-ANSWER-UNCHECKED rulesets [$bad_rulesets]" "$got" \
    "2|refusing: eventloops/upstroke's ruleset state read back as [$bad_rulesets]"
  [[ "$got" == *"#999"* ]] \
    && error "MUT-RETARGET-ANSWER-UNCHECKED: the audit judged a pull request on a ruleset state it could not read"
done
# The four real answers still run, so the case above is about the shape and not about the check.
for good_rulesets in "0 0" "0 1" "1 0" "1 1"; do
  got="$(ruleset_answer_run "$good_rulesets")"
  contains "MUT-RETARGET-ANSWER-UNCHECKED rulesets [$good_rulesets]" "$got" "#999"
  [[ "$got" == *"ruleset state read back"* ]] \
    && error "MUT-RETARGET-ANSWER-UNCHECKED: [$good_rulesets] was refused as a ruleset state"
done

# With no arguments the audit walks the open pull requests. An unread list became an empty one: a
# header, no rows, exit 0 -- which is what "every pull request was audited and none was ready"
# looks like. It must refuse instead.
got="$(STUB_PULLS_STATUS=1 run_stub)"
contains MUT-PR-LIST-SUPPRESSED "$got" "refusing: could not list"
[[ "$got" == *"#999"* ]] && error "MUT-PR-LIST-SUPPRESSED: the audit carried on past a failed listing"
# The stub's readable listing is empty, which is a real answer and must not refuse.
got="$(run_stub)"
[[ "$got" == *"refusing: could not list"* ]] && error "MUT-PR-LIST-SUPPRESSED: an empty listing was reported as a failed one"

# An empty --reviewer is a supplied value, not an absent one, and it is checked like any other.
got="$(run_stub --reviewer "" 999)"
contains MUT-REVIEWER-EMPTY-OVERRIDE "$got" "2|refusing: --reviewer [] is not a GitHub login."
[[ "$got" == *GH-REACHED* || "$got" == *"#999"* ]] && error "MUT-REVIEWER-EMPTY-OVERRIDE: an empty --reviewer reached the audit"
# So is an environment variable that is set and empty: a wrapper whose own variable was unset
# exports one, and reading it as "nothing was supplied" hands the run to the repository's owner.
got="$(UPSTROKE_REVIEW_AUTHOR= run_stub 999)"
contains MUT-REVIEWER-EMPTY-OVERRIDE "$got" "refusing: UPSTROKE_REVIEW_AUTHOR=[] is not a GitHub login."
# Unset is still unset, and the User owner still stands in: this narrows nothing that worked.
got="$(run_stub 999)"
[[ "$got" == *refusing* ]] && error "MUT-REVIEWER-EMPTY-OVERRIDE: an unset UPSTROKE_REVIEW_AUTHOR was read as supplied"

# A lone hyphen is an option too. `-?*` required a character after it, so `--ready-label -`
# labelled a pull request `-` and enqueued it.
for lone in - -h --help; do
  got="$(run_stub --ready-label "$lone" 999)"
  contains MUT-OPTION-LONE-HYPHEN "$got" "2|refusing: --ready-label needs a label name, got the option [$lone]"
  got="$(run_stub --reviewer "$lone" 999)"
  contains MUT-OPTION-LONE-HYPHEN "$got" "2|refusing: --reviewer needs a login, got the option [$lone]"
done
expect MUT-OPTION-LONE-HYPHEN "$(option_like - && echo yes || echo no)" yes
expect MUT-OPTION-LONE-HYPHEN "$(option_like queue-me && echo yes || echo no)" no

# A parse that could not be completed, through main. The old parser printed tab-separated rows and
# an `END` row to say the stream was whole, and a review's own strings could write an `END`: one
# recording `base_sha: "<a real base commit>\nEND\t-\t0"` printed the completeness marker itself,
# the parser died on the next field, and the audit read a finished parse and a clean PASS out of a
# process that exited 1. There is no marker to forge now -- completeness is the exit status, plus
# a record count the parser computed -- and `PYTHONIOENCODING=ascii`, the environment that used to
# kill the parser halfway through its output, no longer reaches the output at all: the result is
# rendered to bytes this program encodes itself.
#
# So the same two reviews are run here for the opposite assertion. Each must parse WHOLE, keep its
# finding, and block for having one; the review that writes `END` into its own base must have that
# base refused as a commit and nothing else.
printf '```json\n{"verdict":"PASS","findings":[{"id":"A-DEFERRABL\xc3\x89","severity":"P3"}]}\n```\n' \
  > "$tmp/truncating-review.md"
got="$(STUB_REVIEW_BODY="$tmp/truncating-review.md" STUB_COMMENTS_STATUS=0 PYTHONIOENCODING=ascii run_stub 999)"
contains MUT-REVIEW-PARSE-TRUNCATED "$got" "NOT-READY"
contains MUT-REVIEW-PARSE-TRUNCATED "$got" "pass-with-findings"
[[ "$got" == *review-parse-incomplete* ]] \
  && error "MUT-REVIEW-PARSE-TRUNCATED: a whole parse was reported as truncated"
[[ "$got" == *review-parse-failed* ]] \
  && error "MUT-REVIEW-PARSE-TRUNCATED: a whole parse was reported as failed"
# The same review with the environment left alone, so the case above is about the encoding and not
# about the review.
got="$(STUB_REVIEW_BODY="$tmp/truncating-review.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-REVIEW-PARSE-TRUNCATED "$got" "pass-with-findings"
contains MUT-META-FIELD-COLLAPSE "$got" "review-records-no-reviewed-sha"

printf '```json\n{"base_sha":"%s\\nEND\\t-\\t0","verdict":"PASS","findings":[{"id":"A-DEFERRABL\xc3\x89","severity":"P3"}]}\n```\n' \
  5157509000000000000000000000000000000002 > "$tmp/forging-review.md"
got="$(STUB_REVIEW_BODY="$tmp/forging-review.md" STUB_COMMENTS_STATUS=0 PYTHONIOENCODING=ascii run_stub 999)"
contains MUT-REVIEW-FORGES-PROTOCOL "$got" "NOT-READY"
contains MUT-REVIEW-FORGES-PROTOCOL "$got" "pass-with-findings"
[[ "$got" == *review-parse-incomplete* ]] \
  && error "MUT-REVIEW-FORGES-PROTOCOL: a forged marker shortened the parse"

# --- the P3 rule, and the PASS rule it replaces, through main -----------------------------------
# `verdict-not-pass` demanded a PASS from a lane whose reviews file P3s, so it could be reached only
# by looping reviews until one returned nothing. It is deleted, and the rule now is a COUNT: no P0,
# P1 or P2, and three P3s or fewer -- three UNWITNESSED ones, because a P3 carrying a failing test,
# reproduction or mutation witness is fixed whatever the count and is already blocked in every lane.
#
# These run the whole script against the stub, because the count is made inside `audit_one` and no
# call on a helper can reach it. They assert the LANE RULE'S OWN BLOCKER and not the READY verdict:
# a review the stub can serve has no ledger row and no filed finding behind it, so every one of
# these pull requests is NOT-READY on `no-row:` whatever the count says, and the question this rule
# answers is whether `unwitnessed-p3s` is among the blockers.
p3_review() {  # p3_review FILE FINDING-JSON...: a CHANGES_REQUIRED review carrying those findings
  local out="$1" sep='' one
  shift
  printf '```json\n{"verdict":"CHANGES_REQUIRED","findings":[' > "$out"
  for one in "$@"; do
    printf '%s%s' "$sep" "$one" >> "$out"
    sep=','
  done
  printf ']}\n```\n' >> "$out"
}
unwitnessed_p3='{"id":"P3-%s","severity":"P3"}'
p3_review "$tmp/p3-three.md" \
  "$(printf "$unwitnessed_p3" a)" "$(printf "$unwitnessed_p3" b)" "$(printf "$unwitnessed_p3" c)"
p3_review "$tmp/p3-four.md" \
  "$(printf "$unwitnessed_p3" a)" "$(printf "$unwitnessed_p3" b)" "$(printf "$unwitnessed_p3" c)" \
  "$(printf "$unwitnessed_p3" d)"
p3_review "$tmp/p3-one-witnessed.md" '{"id":"P3-w","severity":"P3","witness":"a failing test"}'
# THE CASE THAT SEPARATES THE TWO COUNTS, and the only one that does: three unwitnessed P3s beside a
# witnessed one. Counted as four the tolerance is exceeded; counted as the three the rule is written
# over, it is not -- and the witnessed one blocks on its own terms either way, which is why a fixture
# carrying a witnessed P3 ALONE cannot tell the two apart.
witnessed_p3='{"id":"P3-w","severity":"P3","mutation":"a surviving mutant"}'
p3_review "$tmp/p3-three-plus-witnessed.md" \
  "$(printf "$unwitnessed_p3" a)" "$(printf "$unwitnessed_p3" b)" "$(printf "$unwitnessed_p3" c)" \
  "$witnessed_p3"
p3_review "$tmp/p3-four-plus-witnessed.md" \
  "$(printf "$unwitnessed_p3" a)" "$(printf "$unwitnessed_p3" b)" "$(printf "$unwitnessed_p3" c)" \
  "$(printf "$unwitnessed_p3" d)" "$witnessed_p3"

p3_run() {  # p3_run BRANCH FILE: the audit's line for a pull request on that branch and review
  STUB_BRANCH="$1" STUB_REVIEW_BODY="$2" STUB_COMMENTS_STATUS=0 run_stub 999
}
got="$(p3_run fix-P3/correctness_a-slug "$tmp/p3-three.md")"
contains MUT-P3-COUNT-UNBOUNDED "$got" "fix-p3"
[[ "$got" == *unwitnessed-p3s* ]] \
  && error "MUT-P3-COUNT-UNBOUNDED: three unwitnessed P3s were blocked by the tolerance: [$got]"
got="$(p3_run fix-P3/correctness_a-slug "$tmp/p3-four.md")"
contains MUT-P3-COUNT-UNBOUNDED "$got" "unwitnessed-p3s:4-over-3"
contains MUT-P3-COUNT-UNBOUNDED "$got" "NOT-READY"
# A witnessed P3 is one P3 -- inside the tolerance by count -- and blocks anyway. `flags & 1` is the
# parser's word for a witness field, and it is cleared P3s that the tolerance counts.
got="$(p3_run fix-P3/correctness_a-slug "$tmp/p3-one-witnessed.md")"
contains MUT-P3-COUNT-UNBOUNDED "$got" "witnessed:P3-w"
contains MUT-P3-COUNT-UNBOUNDED "$got" "NOT-READY"
[[ "$got" == *unwitnessed-p3s* ]] \
  && error "MUT-P3-COUNT-UNBOUNDED: a witnessed P3 was counted as an unwitnessed one: [$got]"
# Three unwitnessed P3s and a witnessed one: four findings, three of them counted. The witnessed one
# blocks, and the tolerance does not -- which is the whole of "three UNWITNESSED P3s".
got="$(p3_run fix-P3/correctness_a-slug "$tmp/p3-three-plus-witnessed.md")"
contains MUT-P3-COUNT-UNBOUNDED "$got" "witnessed:P3-w"
[[ "$got" == *unwitnessed-p3s* ]] \
  && error "MUT-P3-COUNT-UNBOUNDED: a witnessed P3 was counted towards the tolerance: [$got]"
# And with a fourth unwitnessed one the blocker names FOUR and not five, so the number it reports is
# the number it counted.
got="$(p3_run fix-P3/correctness_a-slug "$tmp/p3-four-plus-witnessed.md")"
contains MUT-P3-COUNT-UNBOUNDED "$got" "unwitnessed-p3s:4-over-3"
# bulk-fix-P3/ shares the lane and so shares the rule.
got="$(p3_run bulk-fix-P3/a-batch "$tmp/p3-four.md")"
contains MUT-P3-COUNT-UNBOUNDED "$got" "unwitnessed-p3s:4-over-3"
# And it is the fix-p3 LANE'S rule: four deferrable P3s on a feature branch are four filed findings.
got="$(p3_run feature/a-slug "$tmp/p3-four.md")"
[[ "$got" == *unwitnessed-p3s* ]] \
  && error "MUT-P3-COUNT-UNBOUNDED: the P3 tolerance was applied outside the fix-p3 lane: [$got]"
# The rule it replaces is gone, in the audit and on the wire: a P3 lane with a CHANGES_REQUIRED
# verdict used to block on `verdict-not-pass` alone, which no review could ever clear.
got="$(p3_run fix-P3/correctness_a-slug "$tmp/p3-three.md")"
[[ "$got" == *verdict-not-pass* ]] \
  && error "MUT-P3-COUNT-UNBOUNDED: verdict-not-pass still blocks a P3 lane: [$got]"
expect MUT-P3-COUNT-UNBOUNDED "$(code_lines | grep -c 'verdict-not-pass' || true)" 0

# --- a branch outside the vocabulary, through main ----------------------------------------------
# The catch-all is gone from the audit too, and this is what a pull request carrying a name outside
# the vocabulary now gets: a row, a blocker, NOT-READY, and the run carries on. It must not be given
# a lane, and it must not stop the audit -- one unknown name is not a reason to leave every other
# pull request unjudged.
got="$(STUB_BRANCH=codex/findings-p3-7d2d8e9dc74a STUB_REVIEW_BODY="$tmp/p3-three.md" \
  STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-LANE-PREFIX-UNKNOWN-DEFAULTS "$got" "branch-prefix-unknown:codex"
contains MUT-LANE-PREFIX-UNKNOWN-DEFAULTS "$got" "NOT-READY"
contains MUT-LANE-PREFIX-UNKNOWN-DEFAULTS "$got" "is not a known branch prefix"
expect MUT-LANE-PREFIX-UNKNOWN-DEFAULTS "${got%%|*}" 0
# The LANE COLUMN of the row itself, and not the run's output, which carries the vocabulary the
# refusal printed and every lane name in it. `%-14s` pads the column, so a `-` there is the audit
# saying this pull request has no lane; any lane name would appear in exactly that position.
contains MUT-LANE-PREFIX-UNKNOWN-DEFAULTS "$got" '#999  -             '

# --- a lane label is an output, and a stale one is reported and removed --------------------------
# The two reconciliation loops named `lane:feature`, `lane:findings-p1p2` and `lane:findings-p3` by
# hand. Two of those lanes no longer exist, so a label left behind by the old audit would have been
# neither reported nor removed; and the list would have gone stale again the next time the table
# moved. Every `lane:*` label that is not this pull request's lane is reported, whether or not the
# table has ever heard of it.
got="$(STUB_BRANCH=docs/a-slug STUB_LABELS=$'lane:feature\nlane:findings-p3\nready-to-merge' \
  STUB_REVIEW_BODY="$tmp/p3-three.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
# The lane came from the PREFIX and not from either label on the pull request. A label is not bound
# to a commit, so a lane read out of one would let an edit change which severities bind a merge.
contains MUT-LANE-LABEL-INPUT "$got" '#999  docs          '
contains MUT-LANE-LABEL-LIST-BY-HAND "$got" "lane-label-mismatch=lane:feature"
contains MUT-LANE-LABEL-LIST-BY-HAND "$got" "lane-label-mismatch=lane:findings-p3"
[[ "$got" == *"lane-label-mismatch=ready-to-merge"* ]] \
  && error "MUT-LANE-LABEL-LIST-BY-HAND: a label outside the lane: namespace was reported as a lane"
# and the pull request's own lane label is not a mismatch with itself.
got="$(STUB_BRANCH=docs/a-slug STUB_LABELS='lane:docs' \
  STUB_REVIEW_BODY="$tmp/p3-three.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
[[ "$got" == *lane-label-mismatch* ]] \
  && error "MUT-LANE-LABEL-LIST-BY-HAND: a correct lane label was reported as a mismatch: [$got]"

# --- a label name may hold a space, and a space is not its boundary -----------------------------
# The labels arrived as one space-joined field and every reader of that field split it on
# whitespace. `lane:legacy docs` was collected as `lane:legacy`: the real label was never
# identified, so the sweep this audit claims -- every `lane:*` label that is not this pull
# request's -- did not hold, and --apply then asked GitHub to remove a label the pull request does
# not carry. That removal fails, and under `set -e` the run ends there with every pull request
# after it unaudited. Measured on the unrepaired audit: `lane-label-mismatch=lane:legacy`, one
# `--remove-label lane:legacy`, exit 1.
#
# The whole label, in the REPORT. `blockers=` is included so this cannot be satisfied by the
# truncated name: `lane-label-mismatch=lane:legacy blockers=` is what the defect printed.
got="$(STUB_BRANCH=docs/a-slug STUB_LABELS='lane:legacy docs' \
  STUB_REVIEW_BODY="$tmp/p3-three.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$got" "lane-label-mismatch=lane:legacy docs blockers="

# AND IN THE REMOVAL, which is where the run died. This drives --apply against a stub that records
# every `pr edit` argument one per bracket -- so a label holding a space cannot be misread here
# either -- and that REFUSES a removal of a label the pull request does not carry, which is what
# GitHub does. Two pull requests, so "the run carried on" is observed and not assumed.
apply_gh="$tmp/apply-gh"
mkdir -p "$apply_gh"
cat > "$apply_gh/gh" <<'GH'
#!/usr/bin/env bash
head=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
case "$*" in
  "repo view"*)               echo eventloops/upstroke ;;
  *"--jq .owner.login")       echo eventloops ;;
  *"--jq .owner.type")        echo User ;;
  *"/labels?per_page=100"*)   ;;                                 # the repository has none of them yet
  "label create"*)            ;;
  "pr edit"*)                 printf '[%s]' "$@" >> "$STUB_EDIT_LOG"
                              printf '\n' >> "$STUB_EDIT_LOG"
                              # GitHub refuses to take off a label that is not on the pull
                              # request, and that refusal is the finding's whole cost.
                              prev=''
                              for arg in "$@"; do
                                if [[ "$prev" == --remove-label ]]; then
                                  case $'\n'"${STUB_LABELS:-}"$'\n' in
                                    *$'\n'"$arg"$'\n'*) ;;
                                    *) echo "could not remove label: $arg" >&2; exit 1 ;;
                                  esac
                                fi
                                prev="$arg"
                              done ;;
  *rulesets*)                 ;;                                 # no branch ruleset
  *check-runs*)               printf 'upstroke-ci\t10\tsuccess\nupstroke-pr-policy\t11\tsuccess\n' ;;
  *"/pulls?state=open"*)      exit 0 ;;
  *timeline*)                 ;;                                 # no base change
  *"/comments?per_page=100"*) [[ -n "${STUB_REVIEW_BODY:-}" ]] && echo "2026-09-01T00:00:00Z 5001"
                              exit "${STUB_COMMENTS_STATUS:-1}" ;;
  *"/issues/comments/5001 --jq .created_at") echo 2026-09-01T00:00:00Z ;;
  *"/issues/comments/5001 --jq .body")       cat "$STUB_REVIEW_BODY" ;;
  *"--json body"*)            echo "no ledger" ;;
  "pr view"*)                 printf '%s\n%s\nfalse\nCLEAN\nmaster\n%s\n' \
                                "${STUB_BRANCH:-feature/x}" "$head" "$head"
                              stub_labels_n=0
                              if [[ -n "${STUB_LABELS:-}" ]]; then
                                stub_labels_n="$(printf '%s\n' "$STUB_LABELS" | wc -l)"
                              fi
                              printf '%s\n' "$stub_labels_n"
                              if [[ -n "${STUB_LABELS:-}" ]]; then printf '%s\n' "$STUB_LABELS"; fi ;;
  *) echo "GH-UNSTUBBED $*" >&2; exit 97 ;;
esac
GH
chmod +x "$apply_gh/gh"
run_apply() {  # run_apply ARG...: "<exit status>|<output, on one line>", with --apply
  local out status=0
  out="$(PATH="$apply_gh:$PATH" bash scripts/pr-ready-audit.sh --apply "$@" 2>&1)" || status=$?
  printf '%s|%s' "$status" "$(tr '\n' ' ' <<< "$out")"
}
: > "$tmp/apply-edits.log"
got="$(STUB_EDIT_LOG="$tmp/apply-edits.log" STUB_BRANCH=docs/a-slug STUB_LABELS='lane:legacy docs' \
  STUB_REVIEW_BODY="$tmp/p3-three.md" STUB_COMMENTS_STATUS=0 run_apply 999 1000)"
edits="$(tr '\n' ' ' < "$tmp/apply-edits.log")"
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$edits" '[--remove-label][lane:legacy docs]'
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$edits" '[--add-label][lane:docs]'
[[ "$edits" == *'[--remove-label][lane:legacy]'* ]] \
  && error "MUT-LANE-LABEL-SPLIT-ON-SPACES: the removal named the first word of the label: [$edits]"
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$got" "0|"
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$got" "#1000"

# A LABEL THAT MERELY CONTAINS THE READY LABEL IS NOT THE READY LABEL. `[[ " $labels " ==
# *" $ready_label "* ]]` over the joined string answered yes for a single label `x ready-to-merge
# y`, so a NOT-READY pull request carrying one had `ready-to-merge` taken off it -- a removal of a
# label it does not have, which fails and ends the run exactly as the case above does.
: > "$tmp/apply-edits.log"
got="$(STUB_EDIT_LOG="$tmp/apply-edits.log" STUB_BRANCH=docs/a-slug \
  STUB_LABELS=$'lane:docs\nx ready-to-merge y' \
  STUB_REVIEW_BODY="$tmp/p3-three.md" STUB_COMMENTS_STATUS=0 run_apply 999)"
edits="$(tr '\n' ' ' < "$tmp/apply-edits.log")"
[[ "$edits" == *'--remove-label'* ]] \
  && error "MUT-LANE-LABEL-SPLIT-ON-SPACES: a label containing the ready label was taken for it: [$edits]"
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$got" "0|"
# and the ready label itself is still recognised when it really is on the pull request, so the
# case above is about the boundary and not about the test having stopped working.
: > "$tmp/apply-edits.log"
got="$(STUB_EDIT_LOG="$tmp/apply-edits.log" STUB_BRANCH=docs/a-slug \
  STUB_LABELS=$'lane:docs\nready-to-merge' \
  STUB_REVIEW_BODY="$tmp/p3-three.md" STUB_COMMENTS_STATUS=0 run_apply 999)"
edits="$(tr '\n' ' ' < "$tmp/apply-edits.log")"
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$edits" '[--remove-label][ready-to-merge]'
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$got" "0|"

# A COUNT THAT DISAGREES WITH THE NAMES IS A READ THIS CANNOT TRUST. The labels are one per line
# now, and a label name cannot hold a line ending -- so that is checked rather than assumed: the
# count comes over the wire ahead of the names, and a mismatch means one label arrived as two, a
# half of which could be a `lane:` name this would report and try to remove.
got="$(STUB_BRANCH=docs/a-slug STUB_LABELS=$'lane:a\nb' STUB_LABEL_COUNT=1 \
  STUB_REVIEW_BODY="$tmp/p3-three.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-LANE-LABEL-SPLIT-ON-SPACES "$got" "blockers=pr-lookup-failed"
[[ "$got" == *lane-label-mismatch* ]] \
  && error "MUT-LANE-LABEL-SPLIT-ON-SPACES: a label read this could not trust was acted on: [$got]"

# --- one parser, one success condition ----------------------------------------------------------
# Everything below runs `scripts/pr-review-parse.py`, which is what the audit runs. Two readings of
# every case: the JSON rendering, which says what the parser decided, and the NUL rendering, which
# is the payload the audit actually reads.
command -v python3 > /dev/null || command -v python > /dev/null \
  || { echo "test-pr-ready-audit: needs python3 or python to run the review parser" >&2; exit 1; }
parser_python="$(command -v python3 || command -v python)"
review_rows() {  # review_rows FILE: "<status>|<kind>/<head>/<verdict>/<base>/<stray>[;<sev>:<id>:<flags>]..."
  local out status=0
  out="$("$parser_python" scripts/pr-review-parse.py review "$1" 2>/dev/null)" || status=$?
  ((status == 0)) || { printf '%s|' "$status"; return 0; }
  printf '%s|%s' "$status" "$(
    printf '%s' "$out" | jq -j '"\(.kind)/\(.reviewed_sha//"-")/\(.verdict//"-")/\(.base_sha//"-")/\(.stray//"-")",
      ([.findings[] | ";\(.severity):\(.id//"-"):\(.flags)"] | add // "")'
  )"
}
parse_nul() {  # parse_nul SUBCOMMAND FILE [OUT]: "<status>|<the payload, NULs shown as |>"
  local out="${3:-$tmp/nul.out}" status=0
  : > "$out"
  "$parser_python" scripts/pr-review-parse.py "$1" --nul --out "$out" "$2" 2>/dev/null || status=$?
  printf '%s|%s' "$status" "$(tr '\0' '|' < "$out")"
}

# --- the workflow form: a fenced JSON verdict ---------------------------------------------------
cat > "$tmp/json.md" <<'EOF'
Findings workflow review 2/2.

Reviewed head: 4ad962f000000000000000000000000000000001
Base: 5157509000000000000000000000000000000002
Reviewer: gpt-5.6-sol, medium effort. An earlier draft said {"verdict":"PASS"} but see below;
the prose here also mentions a P1 that is not in the object.

Unedited verdict:

```json
{"reviewed_sha":"4ad962f000000000000000000000000000000001","base_sha":"5157509000000000000000000000000000000002","verdict":"CHANGES_REQUIRED","findings":[{"id":"A-DEFERRABLE","severity":"P3","reproduction":null,"witness":false},{"id":"B-WITNESSED","severity":"P2","failing_test":"a_test_that_fails"},{"id":"C-MUST","severity":"P2","correction":"This is a MUST deviation of standards section 7"},{"id":"D-BAD","severity":"P9"},{"id":"E.DOTTED","severity":"P3","location":"src/x.rs:1"}]}
```
EOF
expect "MUT-JSON-SPLIT-BY-REGEX/MUT-NULL-WITNESS/MUT-MUST-UNSEEN/MUT-BAD-SEVERITY-PASSES/MUT-STRAY-TOKEN-UNSEEN/MUT-VERDICT-FROM-PROSE/MUT-QUOTED-JSON-IS-JSON" \
  "$(review_rows "$tmp/json.md")" \
  '0|json/4ad962f000000000000000000000000000000001/CHANGES_REQUIRED/5157509000000000000000000000000000000002/P1;P3:A-DEFERRABLE:0;P2:B-WITNESSED:1;P2:C-MUST:2;ERR:bad-severity:D-BAD:0;P3:E.DOTTED:0'
# The same result as the audit reads it: seven fields, the seventh the finding count, then three
# per finding. The count is the parser's own and the audit checks the payload against it, so a
# payload that stopped arriving is not a shorter findings list.
expect MUT-PARSE-PAYLOAD-COUNT "$(parse_nul review "$tmp/json.md")" \
  '0|review|json|4ad962f000000000000000000000000000000001|CHANGES_REQUIRED|5157509000000000000000000000000000000002|P1|5|P3|A-DEFERRABLE|0|P2|B-WITNESSED|1|P2|C-MUST|2|ERR|bad-severity:D-BAD|0|P3|E.DOTTED|0|'

# --- an incomplete final verdict is not a licence to take the previous one -----------------------
# THE LAST BLOCK IS THE VERDICT, AND WHETHER IT IS WHOLE IS ASKED AFTER IT HAS BEEN IDENTIFIED.
# The pattern this replaces matched only blocks that HOLD A WHOLE OBJECT and then took the last of
# those, which is the two operations in the wrong order: with the final closing fence removed, or
# the final `}`, the real verdict was not a match at all and `[-1]` named the `PASS` quoted above
# it as an example. The severity is written `P\u0031`, as the review that found this wrote it,
# because a JSON escape carries no `P1` for the stray scan to catch -- so nothing else was left to
# block: the parser reported PASS with zero findings and exit 0, and the audit READY with a merge
# call. A truncated review has no verdict; it does not have its previous verdict.
revived_head=4ad962f000000000000000000000000000000001
revived_base=5157509000000000000000000000000000000002
example_pass() {  # example_pass: the clean `PASS` block these revivals all reached back to
  printf 'An earlier pass, kept as an example of the shape:\n\n'
  printf '```json\n{"reviewed_sha":"%s","base_sha":"%s","verdict":"PASS","findings":[]}\n```\n\n' \
    "$revived_head" "$revived_base"
}
{ printf 'Reviewed head: %s\n\nUnedited verdict:\n\n```json\n' "$revived_head"
  printf '{"reviewed_sha":"%s","base_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P\\u0031"}]}' \
    "$revived_head" "$revived_base"
  printf '\n'; } > "$tmp/revived-body.txt"
{ cat "$tmp/revived-body.txt"; printf '```\n'; } > "$tmp/revived-whole.md"
cp "$tmp/revived-body.txt" "$tmp/revived-no-fence.md"
{ head -c -2 "$tmp/revived-body.txt"; printf '\n```\n'; } > "$tmp/revived-no-brace.md"
# Whole, the real verdict is the one that counts and its escaped severity is a finding.
expect MUT-VERDICT-REVIVED-BY-TRUNCATION "$(review_rows "$tmp/revived-whole.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/$revived_base/-;P1:CRITICAL:0"
# Cut either way, there is no verdict at all -- not the one above it.
for cut in no-fence no-brace; do
  got="$(review_rows "$tmp/revived-$cut.md")"
  [[ "$got" == 0\|* ]] \
    && error "MUT-VERDICT-REVIVED-BY-TRUNCATION [$cut]: a truncated review parsed, got [$got]"
  [[ "$got" == *PASS* ]] \
    && error "MUT-VERDICT-REVIVED-BY-TRUNCATION [$cut]: an earlier PASS was revived, got [$got]"
  # and nothing was written where a caller ignoring the status would read it
  expect "MUT-VERDICT-REVIVED-BY-TRUNCATION [$cut] payload" \
    "$(parse_nul review "$tmp/revived-$cut.md")" '1|'
done
# Through main, because the parser refusing is only half of it: READY and a merge call is what the
# audit did with the revived PASS.
for cut in no-fence no-brace; do
  got="$(STUB_REVIEW_BODY="$tmp/revived-$cut.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
  contains "MUT-VERDICT-REVIVED-BY-TRUNCATION [$cut]" "$got" "review-parse-failed"
  contains "MUT-VERDICT-REVIVED-BY-TRUNCATION [$cut]" "$got" "NOT-READY"
  [[ "$got" == *"verdict=PASS"* ]] \
    && error "MUT-VERDICT-REVIVED-BY-TRUNCATION [$cut]: the audit read PASS out of a truncated review"
done
# The same comment whole still reaches the audit as the blocking review it is, so the cases above
# are about the truncation and not about the fixture.
got="$(STUB_REVIEW_BODY="$tmp/revived-whole.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-VERDICT-REVIVED-BY-TRUNCATION "$got" "open-P1:CRITICAL"
# AND THE SHAPE EVERY ONE OF THESE REVIVALS REACHED BACK INTO: a complete `PASS` example above the
# real verdict. Round 11 got to it through a final block that did not close, round 12 through an
# indented fence, round 13 through a `> ` and through an HTML comment -- four ways to make the
# example the block that was read. The comment offers two places a verdict could be read from, and
# a parse that picks one of them is picking a verdict: there is no result here.
{ example_pass; cat "$tmp/revived-body.txt"; printf '```\n'; } > "$tmp/two-blocks.md"
got="$(review_rows "$tmp/two-blocks.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: a comment offering two verdicts parsed, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: the example PASS was chosen, got [$got]"
expect MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN "$(parse_nul review "$tmp/two-blocks.md")" '1|'
got="$(STUB_REVIEW_BODY="$tmp/two-blocks.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN "$got" "review-parse-failed"
contains MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN "$got" "NOT-READY"
[[ "$got" == *"verdict=PASS"* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: the audit read PASS out of an ambiguous comment"
# A block whose fences the review quotes INSIDE it is still that one block: a `failure_sequence`
# describing a code span carries ``` mid-line, and a close matched anywhere ends the block there.
printf 'Reviewed head: %s\n\n```json\n{"reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"SPAN","severity":"P2","failure_sequence":"append ```a code span``` and a blank line"}]}\n```\n' \
  "$revived_head" "$revived_head" > "$tmp/inner-fence.md"
expect MUT-VERDICT-REVIVED-BY-TRUNCATION "$(review_rows "$tmp/inner-fence.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/-/-;P2:SPAN:0"

# --- an indented fence is a fence, and the block before the verdict is never a candidate ---------
# THE SAME REVIVAL, ARRIVING THROUGH THE RECOGNISER INSTEAD OF THE COMPLETENESS CHECK. CommonMark
# lets an opening fence carry up to three leading spaces, lets the closing fence be indented
# independently of it, and strips the opening fence's indentation from the content
# (https://spec.commonmark.org/0.31.2/#fenced-code-blocks). `^```json` matched none of that: ONE
# SPACE before the real `CHANGES_REQUIRED` block and it stopped being a block at all, so the `PASS`
# quoted above it as an example was selected -- READY and a merge call out of a review carrying a
# P1, whose `"severity":"P\u0031"` the stray scan cannot see either.
#
# Two things are asserted, and the second is the one that matters. Indented fences are recognised;
# AND the verdict is the comment's LAST block, with nothing that could be another block's material
# after it -- so a fence shape this parser gets wrong AGAIN is a refusal, never the block before it.
indented_body() {  # indented_body OPEN-INDENT CLOSE-INDENT FILE
  local open close
  printf -v open '%*s' "$1" ''
  printf -v close '%*s' "$2" ''
  { printf 'Reviewed head: %s\n' "$revived_head"
    printf '\nUnedited verdict:\n\n%s```json\n' "$open"
    printf '%s{"reviewed_sha":"%s","base_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P\\u0031"}]}' \
      "$open" "$revived_head" "$revived_base"
    printf '\n%s```\n' "$close"; } > "$3"
}
# One, two and three spaces, on the opening fence, on the closing fence, and on the two
# independently: every one of them is the real verdict, and none of them is the example above it.
for spec in 0:0 1:1 2:2 3:3 1:0 3:0 0:1 0:3 1:3 3:1; do
  indented_body "${spec%%:*}" "${spec##*:}" "$tmp/indent-$spec.md"
  expect "MUT-VERDICT-REVIVED-BY-INDENT [open ${spec%%:*}, close ${spec##*:}]" \
    "$(review_rows "$tmp/indent-$spec.md")" \
    "0|json/$revived_head/CHANGES_REQUIRED/$revived_base/-;P1:CRITICAL:0"
done
# Through main, because the parser reading it right is only half of it: READY with a merge call is
# what the audit did with the revived PASS.
for spec in 1:1 2:2 3:3; do
  got="$(STUB_REVIEW_BODY="$tmp/indent-$spec.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
  contains "MUT-VERDICT-REVIVED-BY-INDENT [$spec]" "$got" "open-P1:CRITICAL"
  contains "MUT-VERDICT-REVIVED-BY-INDENT [$spec]" "$got" "NOT-READY"
  [[ "$got" == *"verdict=PASS"* ]] \
    && error "MUT-VERDICT-REVIVED-BY-INDENT [$spec]: the audit read PASS out of an indented verdict"
done
# A fence longer than three backticks opens and closes a block, and a closing fence must be at
# least as long as the one it closes -- so a shorter run inside the block is content, and a block
# the comment never closes is a failed parse rather than the block before it.
printf 'Reviewed head: %s\n\n````json\n{"reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"LONG","severity":"P2","failure_sequence":"a line reading ``` on its own"}]}\n````\n' \
  "$revived_head" "$revived_head" > "$tmp/long-fence.md"
expect MUT-VERDICT-REVIVED-BY-INDENT "$(review_rows "$tmp/long-fence.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/-/-;P2:LONG:0"
# THE RULE THAT DOES NOT DEPEND ON THE RECOGNISER. A block that follows the verdict is not a
# reason to reach back past it, whatever that block is -- and with the example `PASS` sitting above
# it, reaching back is exactly what there is to catch.
{ example_pass; cat "$tmp/indent-0:0.md"
  printf '\n```text\nA note appended under the verdict.\n```\n'; } > "$tmp/block-after.md"
got="$(review_rows "$tmp/block-after.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-REVIVED-BY-INDENT: a comment whose last block is not its verdict parsed, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-VERDICT-REVIVED-BY-INDENT: an earlier PASS was revived past a later block, got [$got]"
# And a `PASS` written in every spelling of a block this parser is NOT sure about, which is the
# case that has to hold when the recogniser is wrong again. Four spaces is CommonMark's indented
# code block and not a fence; a tilde fence is a fence this comment's own fences are not; the bare
# form's object opener is neither -- AND ITS OPENER MAY CARRY WHITESPACE, which is the spelling
# that walked past an enumerated tail check; a `> ` puts a real fenced block inside a blockquote,
# where the recogniser saw nothing at all; an HTML comment hides a fenced block from the reader
# while leaving it a block to the scanner; a list marker indents one by two spaces. Each of them
# beside the verdict is a refusal, and none of them is a PASS.
for trailing in '    ```json%b    {"verdict":"PASS","findings":[]}%b    ```' \
                '~~~json%b{"verdict":"PASS","findings":[]}%b~~~' \
                '{"role_understanding":"x","verdict":"PASS","findings":[]}%b%b' \
                '{ "role_understanding":"x","verdict":"PASS","findings":[]}%b%b' \
                '{%b  "role_understanding":"x","verdict":"PASS","findings":[]}%b' \
                '> ```json%b> {"verdict":"PASS","findings":[]}%b> ```' \
                '<!--%b```json%b{"verdict":"PASS","findings":[]}%b```%b-->' \
                '- an example:%b%b  ```json%b  {"verdict":"PASS","findings":[]}%b  ```' \
                '```json%b{"verdict":"PASS","findings":[]}%b```'; do
  { cat "$tmp/indent-0:0.md"; printf "\n$trailing\n" '
' '
' '
' '
'; } > "$tmp/tail-material.md"
  got="$(review_rows "$tmp/tail-material.md")"
  [[ "$got" == 0\|* ]] \
    && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN [$trailing]: a comment carrying a second verdict parsed, got [$got]"
  [[ "$got" == *PASS* ]] \
    && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN [$trailing]: a second PASS was chosen, got [$got]"
  expect "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN payload" "$(parse_nul review "$tmp/tail-material.md")" '1|'
  # and through main, because the parser refusing is only half of it
  got="$(STUB_REVIEW_BODY="$tmp/tail-material.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
  contains "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN [$trailing]" "$got" "review-parse-failed"
  [[ "$got" == *"verdict=PASS"* ]] \
    && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN [$trailing]: the audit read PASS out of it"
done
# The same spellings BEFORE the verdict, because a second place is a second place wherever it is:
# an enumerated tail check only ever looked after the block it had already chosen.
for leading in '```json%b{"verdict":"PASS","findings":[]}%b```' \
               '{ "role_understanding":"x","verdict":"PASS","findings":[]}%b%b' \
               '> ```json%b> {"verdict":"PASS","findings":[]}%b> ```' \
               '~~~json%b{"verdict":"PASS","findings":[]}%b~~~' \
               '<!--%b```json%b{"verdict":"PASS","findings":[]}%b```%b-->'; do
  { printf "$leading\n\n" '
' '
' '
' '
'; cat "$tmp/indent-0:0.md"; } > "$tmp/head-material.md"
  got="$(review_rows "$tmp/head-material.md")"
  [[ "$got" == 0\|* ]] \
    && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN [$leading, before]: a comment carrying a second verdict parsed, got [$got]"
  [[ "$got" == *PASS* ]] \
    && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN [$leading, before]: a second PASS was chosen, got [$got]"
done
# Detection reads the same structure: a comment whose ONLY verdict block is indented is the
# workflow form, not prose -- the prose parser would read its `VERDICT:` line from outside the
# object and miss every severity the object spells with an escape.
printf 'Reviewed head: %s\n\n   ```json\n   {"reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"ONLY","severity":"P\\u0031"}]}\n   ```\n' \
  "$revived_head" "$revived_head" > "$tmp/indent-only.md"
expect MUT-VERDICT-REVIVED-BY-INDENT "$(review_rows "$tmp/indent-only.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/-/-;P1:ONLY:0"
# A `VERDICT:` LINE OUTSIDE THE OBJECT IS MATERIAL TOO. The same comment with `VERDICT: PASS`
# written under it says two different things -- the object blocks, the line approves -- and the
# two forms disagree about which one a reader sees. Choosing between them is what the prose fall
# back did; there is nothing to choose here, because the comment does not read.
{ cat "$tmp/indent-only.md"; printf '\nVERDICT: PASS\n'; } > "$tmp/verdict-line-outside.md"
got="$(review_rows "$tmp/verdict-line-outside.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: a comment carrying a verdict object and a verdict line parsed, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: the line outside the object was taken, got [$got]"
# and written ABOVE the object, where a tail check never looked
{ printf 'VERDICT: PASS\n\n'; cat "$tmp/indent-only.md"; } > "$tmp/verdict-line-above.md"
got="$(review_rows "$tmp/verdict-line-above.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: a verdict line above the object parsed, got [$got]"
# And what is OUTSIDE the verdict is found by POSITION. `text.replace(block, "")` removes every
# copy of the block's text and not the one that was read -- and an indented block's content, with
# its indentation stripped, is no longer a substring of the comment at all, so it removes nothing
# and the object's own severities are counted as tokens loose in the prose. A review with one
# recorded finding would go to a person carrying a stray `P1` it does not have.
{ printf 'Reviewed head: %s\n\n  ```json\n  {\n' "$revived_head"
  printf '    "reviewed_sha": "%s",\n    "verdict": "CHANGES_REQUIRED",\n' "$revived_head"
  printf '    "findings": [{"id": "INSIDE", "severity": "P1"}]\n  }\n  ```\n'; } > "$tmp/indent-stray.md"
expect MUT-VERDICT-REVIVED-BY-INDENT "$(review_rows "$tmp/indent-stray.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/-/-;P1:INSIDE:0"
# A tilde fence is a block -- it has to be, or a ``` inside one would be read as a fence of the
# comment's own -- and it is NOT a verdict, because the workflow writes backticks and narrowing is
# the safe direction. It is still the workflow form, though: reading a comment whose only object is
# tilde-fenced as PROSE would take its `VERDICT:` line from outside the object and miss every
# severity the object spells with an escape, which is the defect a detection that cannot fail was
# written to end. So it is a refusal, not a PASS.
printf 'Reviewed head: %s\n\n~~~json\n{"reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"TILDE","severity":"P\\u0031"}]}\n~~~\n\nVERDICT: PASS\n' \
  "$revived_head" "$revived_head" > "$tmp/tilde-only.md"
got="$(review_rows "$tmp/tilde-only.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-REVIVED-BY-INDENT: a tilde-fenced verdict was given a verdict, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-VERDICT-REVIVED-BY-INDENT: a tilde-fenced review was read as prose and passed, got [$got]"
# The older bare form is held to the same rule: what follows the object it was read from may not be
# another block's material either.
printf 'Reviewed head: %s\n\n{"role_understanding":"x","reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"BARE","severity":"P2"}]}\n' \
  "$revived_head" "$revived_head" > "$tmp/bare-whole.md"
expect MUT-VERDICT-REVIVED-BY-INDENT "$(review_rows "$tmp/bare-whole.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/-/-;P2:BARE:0"
{ cat "$tmp/bare-whole.md"; printf '\n```text\nA note appended under the verdict.\n```\n'; } \
  > "$tmp/bare-tail.md"
got="$(review_rows "$tmp/bare-tail.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-REVIVED-BY-INDENT: a bare verdict with block material after it parsed, got [$got]"

# THE FORM IS THE COMMENT'S OWN MARKER, and neither parser is what is left when the other fails.
# A comment carrying the prose form's marker AND a verdict block claims to be both forms, and the
# two do not agree about the same review: this is finding 2's shape with the fence spelled so the
# recogniser DOES see it, which is the half that must refuse for the same reason the half it
# cannot see does.
printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n```json\n{"reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"BOTH","severity":"P\\u0031"}]}\n```\n\nVERDICT: PASS\n' \
  "$revived_head" "$revived_head" > "$tmp/both-forms.md"
got="$(review_rows "$tmp/both-forms.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: a comment claiming both forms parsed, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: a comment claiming both forms was read as prose, got [$got]"
# And the other half of "detection is not a fall back": a comment with NO prose marker and no
# verdict block is the workflow form with nothing to read, which is a refusal. "I could not read
# the JSON, so I will try prose" is how a blocking review became a PASS, and there is no longer a
# road from zero blocks to the prose parser.
printf 'Reviewed head: %s\n\nNo verdict block anywhere.\n\nVERDICT: PASS\n' \
  "$revived_head" > "$tmp/no-block-no-marker.md"
got="$(review_rows "$tmp/no-block-no-marker.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: a workflow comment with no verdict block parsed, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-VERDICT-AMBIGUOUS-BLOCK-CHOSEN: a comment with no verdict block fell back to prose, got [$got]"

# A pretty-printed object with "}, {" between findings is the same object to a parser.
cat > "$tmp/pretty.md" <<'EOF'
Reviewed head: 4ad962f000000000000000000000000000000001

```json
{
  "reviewed_sha": "4ad962f000000000000000000000000000000001",
  "verdict": "CHANGES_REQUIRED",
  "findings": [
    {"id": "FIRST", "severity": "P3"}, {"id": "SECOND", "severity": "P1"}
  ]
}
```
EOF
expect MUT-JSON-SPLIT-BY-REGEX "$(review_rows "$tmp/pretty.md")" \
  '0|json/4ad962f000000000000000000000000000000001/CHANGES_REQUIRED/-/-;P3:FIRST:0;P1:SECOND:0'

# --- a repeated name is two readings, and two readings do not resolve ---------------------------
# `json.loads` KEEPS THE LAST OCCURRENCE OF A REPEATED NAME
# (https://docs.python.org/3/library/json.html#repeated-names-within-an-object), so one fenced
# object could carry a `findings` array with a P1 in it AND a second `"findings":[]` after it, and
# the blocking array was discarded before validation ever saw it. Nothing else was left to block:
# the object IS the recognised verdict block, so its severities are inside it rather than loose in
# the prose the stray scan reads -- and the severity is written `P\u0031` here, as the review that
# found this wrote it, because a JSON escape carries no `P1` for that scan to catch either. PASS,
# zero findings, exit 0, READY and a merge call, out of a review that blocks.
#
# THE DECODE IS WHERE THIS CLOSES, AND IT CLOSES AT EVERY DEPTH. The hook that refuses a repeated
# name is the object's CONSTRUCTOR, not a check run over a decoded object -- a check like that
# would be asking the decoded dict what the text said, which is the question the duplicate already
# answered wrongly. `json.loads` calls it for every object it builds, so the top level, each
# finding, and anything nested under a finding are one rule. Each case below is paired with the
# SAME OBJECT NAMING THE KEY ONCE, which must parse and must still block: a refusal that fires on
# the fixture rather than on the repetition is not this test.
repeated_object() {  # repeated_object OBJECT FILE: OBJECT as the comment's one fenced verdict
  { printf 'Reviewed head: %s\n\nUnedited verdict:\n\n```json\n' "$revived_head"
    printf '%s' "$1"
    printf '\n```\n'; } > "$2"
}
parse_why() {  # parse_why FILE: what one refused parse said on stderr, and nothing it said on stdout
  "$parser_python" scripts/pr-review-parse.py review "$1" 2>&1 > /dev/null || true
}
# The shape the finding reported, and its control: valid head and base, `"verdict":"PASS"`, a
# findings array carrying a P1, and then the second `findings` that erased it.
dup_finding='{"id":"CRITICAL","severity":"P\u0031"}'   # the escape the review that found this wrote: no `P1` for the stray scan to catch
repeated_object \
  "{\"reviewed_sha\":\"$revived_head\",\"base_sha\":\"$revived_base\",\"verdict\":\"PASS\",\"findings\":[$dup_finding],\"findings\":[]}" \
  "$tmp/dup-findings.md"
repeated_object \
  "{\"reviewed_sha\":\"$revived_head\",\"base_sha\":\"$revived_base\",\"verdict\":\"PASS\",\"findings\":[$dup_finding]}" \
  "$tmp/one-findings.md"
# THE CONTROL FIRST, so the cases below cannot pass on a fixture this parser refuses anyway: one
# `findings` key and the P1 is read, recorded and blocking.
expect MUT-JSON-REPEATED-NAME-CHOSEN "$(review_rows "$tmp/one-findings.md")" \
  "0|json/$revived_head/PASS/$revived_base/-;P1:CRITICAL:0"
# The same object naming `findings` twice has no result at all -- not the empty array, not the
# blocking one, and not a PASS.
got="$(review_rows "$tmp/dup-findings.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: an object naming findings twice parsed, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: a repeated name left a PASS standing, got [$got]"
expect MUT-JSON-REPEATED-NAME-CHOSEN "$(parse_nul review "$tmp/dup-findings.md")" '1|'
# AND IT REFUSED FOR THE REASON THIS CASE IS ABOUT. A parse that fails here because some other
# rule of this file moved is not a witness to anything; the diagnostic names the repeated key.
contains MUT-JSON-REPEATED-NAME-CHOSEN "$(parse_why "$tmp/dup-findings.md")" \
  "names 'findings' twice"
# A NAME IS THE NAME THE DECODER BUILDS, not the characters the comment spells it with. Written
# with an escape the second key is the same key, and a guard looking for the literal `"findings"`
# twice would not see it.
repeated_object \
  "{\"verdict\":\"PASS\",\"findings\":[$dup_finding],\"finding\\u0073\":[]}" \
  "$tmp/dup-escaped.md"
got="$(review_rows "$tmp/dup-escaped.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: an escape-spelled repeated name parsed, got [$got]"
contains MUT-JSON-REPEATED-NAME-CHOSEN "$(parse_why "$tmp/dup-escaped.md")" "names 'findings' twice"
# ONE LEVEL DOWN THE SAME SHAPE ERASES A WITNESS instead of the findings list. A finding recording
# a failing test blocks in every lane; the same finding naming `failing_test` twice, the second
# time `null`, is a bare P3 the audit defers. The control is the same finding naming it once.
repeated_object \
  '{"verdict":"PASS","findings":[{"id":"WITNESSED","severity":"P3","failing_test":"a_test_that_fails","failing_test":null}]}' \
  "$tmp/dup-witness.md"
repeated_object \
  '{"verdict":"PASS","findings":[{"id":"WITNESSED","severity":"P3","failing_test":"a_test_that_fails"}]}' \
  "$tmp/one-witness.md"
expect MUT-JSON-REPEATED-NAME-CHOSEN "$(review_rows "$tmp/one-witness.md")" \
  '0|json/-/PASS/-/-;P3:WITNESSED:1'
got="$(review_rows "$tmp/dup-witness.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: a finding naming a witness field twice parsed, got [$got]"
contains MUT-JSON-REPEATED-NAME-CHOSEN "$(parse_why "$tmp/dup-witness.md")" \
  "names 'failing_test' twice"
# AND AT ANY DEPTH BELOW THAT. `finding()` walks a finding's keys whole and `MUST_KEY` matches
# their NAMES, so the set of names that change a verdict is not a list anybody can keep: the rule
# is every name, everywhere, and a nested object is where a rule written for the top level stops.
repeated_object \
  '{"verdict":"PASS","findings":[{"id":"DEEP","severity":"P3","detail":{"note":{"must_fix":"a MUST deviation","must_fix":null}}}]}' \
  "$tmp/dup-deep.md"
repeated_object \
  '{"verdict":"PASS","findings":[{"id":"DEEP","severity":"P3","detail":{"note":{"must_fix":"a MUST deviation"}}}]}' \
  "$tmp/one-deep.md"
expect MUT-JSON-REPEATED-NAME-CHOSEN "$(review_rows "$tmp/one-deep.md")" \
  '0|json/-/PASS/-/-;P3:DEEP:0'
got="$(review_rows "$tmp/dup-deep.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: a repeated name three objects down parsed, got [$got]"
contains MUT-JSON-REPEATED-NAME-CHOSEN "$(parse_why "$tmp/dup-deep.md")" "names 'must_fix' twice"
# The older bare form goes through the same decode, so it is held to the same rule -- and its
# control parses, which is what says the refusal is the repetition and not the missing fence.
printf 'Reviewed head: %s\n\n{"role_understanding":"x","verdict":"PASS","findings":[%s],"findings":[]}\n' \
  "$revived_head" "$dup_finding" > "$tmp/dup-bare.md"
got="$(review_rows "$tmp/dup-bare.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: a bare object naming findings twice parsed, got [$got]"
contains MUT-JSON-REPEATED-NAME-CHOSEN "$(parse_why "$tmp/dup-bare.md")" "names 'findings' twice"
printf 'Reviewed head: %s\n\n{"role_understanding":"x","verdict":"PASS","findings":[%s]}\n' \
  "$revived_head" "$dup_finding" > "$tmp/one-bare.md"
expect MUT-JSON-REPEATED-NAME-CHOSEN "$(review_rows "$tmp/one-bare.md")" \
  "0|json/-/PASS/-/-;P1:CRITICAL:0"
# Through main, because the parser refusing is only half of it: READY with a merge call is what
# the audit did with the erased findings array.
got="$(STUB_REVIEW_BODY="$tmp/dup-findings.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-JSON-REPEATED-NAME-CHOSEN "$got" "review-parse-failed"
contains MUT-JSON-REPEATED-NAME-CHOSEN "$got" "NOT-READY"
[[ "$got" == *"verdict=PASS"* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: the audit read PASS out of an object naming findings twice"
# and the control reaches the audit as the blocking review it is, so the case above is about the
# repeated name and not about the fixture.
got="$(STUB_REVIEW_BODY="$tmp/one-findings.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-JSON-REPEATED-NAME-CHOSEN "$got" "open-P1:CRITICAL"
[[ "$got" == *review-parse-failed* ]] \
  && error "MUT-JSON-REPEATED-NAME-CHOSEN: the control review was refused, got [$got]"
# THE SHAPE, NOT THE INSTANCE -- AND ASKED OF THE DECODE, NEVER OF HOW IT IS WRITTEN. One
# `json.loads` reads review content today and the hook is on it; a second one added without the
# hook is this finding again, in a place no fixture here is pointed at. So what is asserted is the
# class, and this is the EIGHTH guard written for it. Each one before it was wrong, and each in its
# own way:
#
#   * round 0 matched the literal `json.loads(`. `json.loads (text)` is a working, unhooked decoder
#     the scan did not count, and a second decoder carrying the hook turned its count of one red;
#   * round 1 parsed the module and matched the callee path `json.loads` carrying the keyword
#     `object_pairs_hook=one_reading`. The identifier bound to a parameter, and a `cls` override
#     beside the keyword, walked through it; and because `dotted()` discarded a non-name root it
#     REFUSED `factory().json.loads(text)`, a correct parser;
#   * round 2 stopped reading the source and ran the decode -- the right direction -- but it
#     concluded from whatever happened. It read ANY EXCEPTION as a refusal, wrapped `decode` in a
#     copy of the module so `raw_decode` decoded unwatched, called a reader without its keyword-only
#     parameter, and read its answer off a stdout the module shares;
#   * round 3 fixed what round 2 concluded and then asked THE MODULE questions only the standard
#     library and the source can answer: it put the pair through `decoder.decode`, which a module
#     declares however it likes; it took the first frame past the `json` package as the caller, so
#     one `functools.singledispatch` frame hid a live decode; and it chose what to sweep from
#     `co_names`, so a reader decoding through a defaulted parameter was never swept;
#   * round 4 kept a filter on which functions to call, and `setattr(holder, "json", json)` walked
#     around it; and it read a call that reached no decode as green, so a reader decoding only under
#     `mode == "loose"` passed;
#   * round 5 dropped the filter and required a swept call that reached no decoder to run every
#     line -- and it kept the drive and the sweep side by side and let one decide for the other.
#     THE SWEEP SKIPPED EVERY FUNCTION A DRIVE HAD ENTERED, so a branch the drive did not take was
#     run by nobody: `if text.startswith("{"): return json.loads(text, cls=json.JSONDecoder)` at
#     the top of `parse_prose_review`, and a helper decoding unhooked under `UPSTROKE_LOOSE`, both
#     passed, and round 4 had caught the first. Its scanner wrapper counted a refusal only where
#     `raw_decode` raised, so a decoder whose own `decode` drove `scan_once` and caught the refusal
#     passed. A module `__getattr__` returning `json.loads` was called, handed back the decoder, and
#     was accepted, because nothing ran what a call returned. And the same line rule asked a helper
#     nothing calls to run whole on the probe's document, so `if not findings: return 0` was red.
#
# ROUND 6 WAS TOLD TO REPLACE THE SWEEP WITH THE DRIVE OR TO KEEP BOTH WITH COVERAGE PER BRANCH, AND
# TO MEASURE WHICH. BOTH REPLACEMENTS WERE BUILT FROM THIS PROBE -- the sweep switched off, and
# nothing else changed -- and run over the witnesses of all six rounds: 76 mutations of the parser
# and the parser as it is, each through this whole file, each read off the verdict this file asserts
# on the parser. A drive-only probe that still requires every code object in the file to run reds
# nine rows whose right answer is green and which this probe holds green, `print(json.dumps(value))`
# among them, because a helper nothing calls never runs; and it greens three readers that are no
# function of the file's at all -- `reader = json.loads`, `functools.partial(json.loads)`,
# `READERS = (json.loads,)` -- because only the sweep calls them. A drive-only probe that judges
# only the code a drive ran greens 52 rows whose right answer is red: every reader appended to the
# parser that nothing calls. SO BOTH ARE KEPT, AND NEITHER DECIDES FOR THE OTHER. The drive and the
# sweep are two ways of making MODULE's code run; what is concluded is decided by what ran, and by
# nothing else.
#
# AND ROUND 6 STILL CHOSE WHAT TO CALL FROM A LIST, and each of its two reviews executed an escape
# from it -- neither a decode the probe misread, both a place it never looked. It held a
# module-level name, a class body and a module-level container, and a class body only for the
# functions MODULE wrote, so `read = staticmethod(json.loads)` in a class was never called. And it
# called everything with its defaults as written, so `def extra_review_reader(text,
# options={"object_pairs_hook": one_reading})` refused under the sweep and returns PASS for a caller
# passing `options={}` -- a reader MASTER'S GREP REFUSED, which made the round a regression. Every
# round before it was shown one more shape its list did not reach, and a list over an open set is
# never finished. So what is called names no place a callable sits, and what a default could take
# away is not recognised from how the default is written: it is TAKEN AWAY, and what the decode does
# then is the answer.
#
# So this guard states WHAT MAY BE CONCLUDED, AND FROM WHAT, and implements exactly that:
#
#   * WHAT MAKES A SCANNER ACCEPTABLE: it REFUSES A REPEATED NAME, concluded from a PAIRED RESULT --
#     the duplicate and both of its single-name readings -- asked of THE SCANNER THE STANDARD
#     LIBRARY MADE, whose signature the standard library fixes, so nothing MODULE declares is called
#     to put the question.
#   * WHICH SCANNERS: EVERY ONE THE STANDARD LIBRARY MAKES, WHEREVER IT IS MADE. A JSON document is
#     scanned only by a scanner `json.scanner` builds, so every name it builds one under --
#     `make_scanner`, `c_make_scanner`, `py_make_scanner`, and `_json.make_scanner` itself -- is
#     replaced before MODULE is loaded, and the scanner of every decoder that already exists, the
#     default one `json.loads` uses among them, is wrapped where it stands. A scan through one is
#     answered for where it RAN; one MODULE's code built and still holds at the end is asked too.
#     `raw_decode`, `decode`, `scan_once`, a subclass, a context object that is no decoder at all --
#     each is a way of reaching a scanner, and the scanner is where the question is put.
#   * A REFUSAL REACHED IS NOT A REFUSAL DELIVERED. When a scan raises on a document naming
#     something twice, EVERY FRAME OF MODULE'S ABOVE IT MUST LET THE REFUSAL OUT: one that returns
#     or yields while it is on its way through is counted with the unrefusing, whatever it caught
#     and however the catch is written. The one frame excepted is the program's entry point under a
#     drive, because a program reports a refusal by what `main` returns and that is the program's
#     to decide. Round 5 counted only a refusal `raw_decode` raised, and only under the sweep.
#   * AND THE DOCUMENT THE PROGRAM ACTUALLY SCANNED: a scan that RETURNS on a document naming
#     something twice is unrefusing whatever the pair said, because the pair repeats one name at one
#     depth and a document repeats whatever it repeats.
#   * WHERE A SCAN IS ANSWERED FOR: the innermost frame of MODULE's under it; with none, the
#     callable the sweep is calling; with neither, the run the probe was making -- a scan made while
#     MODULE runs is MODULE's, on whichever thread it runs.
#   * WHAT MUST RUN -- PER BRANCH, NOT PER FUNCTION. The probe compiles MODULE's file itself, so
#     every code object in it is known before any of it runs: every function, method, lambda,
#     generator expression and class body, and the module body. For each, EVERY INSTRUCTION THAT
#     COULD RUN CODE OR READ A VALUE OTHER THAN A CONSTANT, reachable from its entry or from any of
#     its handlers, must have run; a code object short of that is UNPROVEN, which is red. What ran
#     is measured with `sys.monitoring`, instruction by instruction, and nothing in it reads a
#     spelling.
#   * HOW IT IS MADE TO RUN. The drive runs the program on its own documents; the sweep calls every
#     callable MODULE's namespaces refer to that nothing has run -- WHATEVER HOLDS IT, found by the
#     garbage collector's own account of what refers to what and never by a list of places -- and
#     whatever those calls return or yield that can be called; and what neither reaches is TAKEN. A
#     callable is called rather than opened, a class MODULE did not write is not walked, and a
#     decoder's own machinery is asked at its scanner. A HANDLER NO RUN ENTERED is entered by
#     raising what it catches at an instruction its body ran, in a repeat of the run that ran it,
#     because any instruction can raise. A BRANCH TAKEN ONE WAY ONLY is turned -- the conditional
#     jump flipped in a copy of the code object -- and the run that took it is repeated, because a
#     branch the probe's documents do not take is exactly where round 5 lost `parse_prose_review`:
#     under that, `if os.environ.get("UPSTROKE_LOOSE")` runs its body, and so does `hook = None if
#     loose else one_reading`, whose other arm is a constant. Taking a branch its condition would
#     not take puts the code in a state its callers may never make, so while a turned run is going
#     the probe's question is put to the code as MODULE wrote it. A code object the turns one at a
#     time leave short is run once more with all of its one-way branches turned together, and a
#     variable MODULE reads from the environment is set, in a repeat of the run that read it, to
#     each string MODULE's code holds. Any run -- turned or not -- that goes past ten seconds is
#     ended where it is, and the question, which runs MODULE's hook, is ended after two and answers
#     UNPROVEN.
#   * A DEFAULT IS AN INPUT, AND A REFUSAL A DEFAULT HOLDS IS NOT THE DECODE'S OWN. A default is
#     what a caller gets by passing nothing, so a refusal it holds is one any caller can take away.
#     At every scan, each default of a function of MODULE's on the stack -- or of the one the sweep
#     is calling -- that holds what the scanner refuses with (its decoder, its hook, the class
#     MODULE wrote for it, what the hook calls) is COPIED WITHOUT IT, and the run is repeated with
#     the copy in place; a default that cannot be copied leaves its function unproven. A default
#     that SELECTS a decode without holding one is set, in a repeat of the run that entered its
#     function, to each value of its own kind the file holds and to the kind's own empty value.
#   * WHAT CANNOT RUN IS NOT ASKED FOR. Code written after a call of something that cannot return --
#     `sys.exit`, or a function of Python with no return in it -- is not required, and that is
#     decided from what the call was seen to call, not from how it is spelled.
#   * THE PROBE OBSERVES ONLY WHAT IT OWNS. Its documents are its own and its answers are held as
#     values; its report goes to a file it names, never to the module's stdout; every function is
#     called by its real signature and run by its real protocol, or reported as skipped; and a
#     failure of the probe's own is never caught -- it ends the probe, and no report is red below.
#
# BOTH HALVES OF THE BOUNDARY, over every scanner the standard library makes. Closed against: a
# scanner that returns on the duplicate, one that is not shown to refuse it, a refusal a frame of
# MODULE's returns over, a refusal that is gone once a default is taken out, and code the probe
# could not make run. Let through: a scanner shown to refuse, whatever reaches it -- a factory, an
# alias, an attribute chain, a namespace attribute, a `functools` wrapper, a `**` unpacking, a
# subclass, a `decode` of its own with a required keyword, a SECOND scanner that genuinely refuses,
# a default that happens to hold the hook the decode does not take from it -- and a helper whose
# every instruction ran without one. What is left either way is measured at the end of this section.
cat > "$tmp/decode-probe.py" <<'DECODEPROBE'
"""Ask every JSON scanner this module's code makes or uses whether it REFUSES A REPEATED NAME -- and
RUN EVERY INSTRUCTION AND TAKE EVERY BRANCH this module's file holds, concluding green only from that.

    decode-probe.py MODULE REPORT-FILE DRIVE...

Each DRIVE is `<argv prefix>|<document>`, the prefix defaulting to `review`: MODULE's own `main` is
run as `<prefix> --out <scratch> <document>` for each. The report is two lines, written to
REPORT-FILE and never to stdout, which belongs to MODULE:

    decoded=<yes|no> unrefusing=<sites> unproven=<sites> skipped=<functions>
    refusing=<sites> drive=<how each drive ended,...> swept=<function:how each call ended,...> forced=<n>

A swept function is called with the probe's document as text, then with one of its readings, then
with a path to the document, until everything written in it has run; its outcomes are joined by `/`.
`forced` counts the runs the probe repeated with something altered: a branch turned, an exception
raised, a variable of the environment set, or a default taken out or changed. An empty
list is written `-`. Exit 0 means the report was written, and any other exit means it was not: a
failure of the probe's own is never caught and never becomes a report.
"""
import builtins, copy, dis, functools, gc, importlib.util, inspect, json.decoder, json.scanner
import opcode, os, signal, sys, tempfile, time, types, weakref

# Coverage is measured with `sys.monitoring`, which Python 3.12 introduced. An older interpreter
# cannot run this probe, and it says so rather than reporting from a measurement it did not take.
if sys.version_info < (3, 12):
    sys.exit("decode-probe: needs sys.monitoring, which Python 3.12 introduced; this is %s"
             % sys.version.split()[0])
# Importing MODULE must not leave a `__pycache__` beside it: this runs against a checkout, and a
# gate that writes into the tree it is judging is a gate that dirties `git status`.
sys.dont_write_bytecode = True

# WHAT MAKES A SCANNER ACCEPTABLE: IT REFUSES A REPEATED NAME. WHAT THAT IS CONCLUDED FROM: A PAIRED
# RESULT, AND NEVER ONE OUTCOME. Raising on a document that names something twice is what a scanner
# refusing the repetition does -- and what one refusing ANYTHING ELSE in that document does, which
# is how round 2 read a hook demanding a finding id as a refusal of the repetition. So every scanner
# is asked three things: the DUPLICATE, and ITS TWO SINGLE-NAME READINGS. Whatever a hook could
# object to in either value on its own, one of the readings carries too. And then:
#
#   * raises on the duplicate, returns on both readings -> REFUSING, the one acceptable answer
#   * returns on the duplicate                          -> UNREFUSING
#   * raises on the duplicate and on a reading too      -> UNPROVEN, which is not acceptable either
DUPLICATE = '{"verdict":"PASS","findings":[{"id":"CRITICAL","severity":"P1"}],"findings":[]}'
READINGS = ('{"verdict":"PASS","findings":[{"id":"CRITICAL","severity":"P1"}]}',
            '{"verdict":"PASS","findings":[]}')

target = os.path.abspath(sys.argv[1])
MODULE = "under_probe"  # the name MODULE is loaded under
probe_file = os.path.abspath(__file__)
report_file = os.path.abspath(sys.argv[2])
drives = sys.argv[3:]
scratch = tempfile.mkdtemp()

answers = {}          # site -> the answers the scanners it used or built gave
swallowed = set()     # functions of MODULE's that returned while a refusal was on its way out
probing = [0]         # > 0 while the probe scans for itself: its question, its own reading
observed = [0]        # scanners reached -- made, or scanned with
built = []            # (site, weak reference) for every scanner MODULE's code makes
inner_of = weakref.WeakKeyDictionary()  # a watched scanner -> the scanner the standard library made
faults = []           # the probe's own failures inside a call of MODULE's, raised again at the end
activity = [None]     # (label, replay) of what the probe is running now: `<module>`, `<drive>`,
                      # or the name of the callable the sweep is calling


def ended(call):
    """How CALL ended -- `returned`, or `raised:<type>` -- as the probe's own record of it."""
    try:
        call()
    except Exception as exc:
        return "raised:" + type(exc).__name__
    return "returned"


# THE PROBE'S OWN READING, BUILT BEFORE ANYTHING IS REPLACED: whether a document the module scans
# names something twice is decided by the standard library's scanner with a hook of the probe's.
class Twice(Exception):
    pass


def names_once(pairs):
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise Twice(name)
        seen.add(name)
    return dict(pairs)


reference = json.decoder.JSONDecoder(object_pairs_hook=names_once)
NEUTRAL = json.decoder.JSONDecoder()  # no hook: what a caller passing nothing gets, and watched
PLAIN = dict(vars(NEUTRAL))           # a decoder's own machinery, as the standard library sets it
real_py = json.scanner.py_make_scanner
real_c = json.scanner.c_make_scanner


def twice(string, idx):
    """Whether the document STRING holds at IDX names something twice, by the probe's own reading."""
    probing[0] += 1
    try:
        reference.scan_once(string, idx)
    except Twice:
        return True
    except Exception:
        return False
    finally:
        probing[0] -= 1
    return False


# MODULE'S CODE IS COMPILED BY THE PROBE, so that every code object the file holds is known before
# any of it runs -- a function, a method, a lambda, a generator expression, a class body, the module
# body -- whether or not anything the module binds will ever reach it.
top = compile(open(target, encoding="utf-8").read(), target, "exec", dont_inherit=True)
tree, parent = [], {}


def walk(code):
    tree.append(code)
    for const in code.co_consts:
        if isinstance(const, types.CodeType):
            parent[const] = code
            walk(const)


walk(top)
mine = set(tree)
alias = {}            # a code object the probe altered -> the one of MODULE's it stands for
installed = {}        # a code object of MODULE's -> the alteration standing in for it now


def pristine():
    """Put every function an alteration of the probe's changed back as MODULE wrote it, for as long
    as the probe asks its question: a branch turned inside a hook is not the hook the module has."""
    changed = []
    if installed:
        changed = [(one, one.__code__) for one in gc.get_objects()
                   if isinstance(one, types.FunctionType) and one.__code__ in alias]
    for function, code in changed:
        function.__code__ = alias[code]

    def restore():
        for function, code in changed:
            function.__code__ = code
    return restore


QUESTION = 2          # seconds the question may take: three documents of a few dozen bytes each
asking = []           # the depth of the probe's bookkeeping each question now being put started at
late = [False]        # the question's time ran out while the probe was recording


def ask(scanner):
    """SCANNER's paired result, asked of the scanner the standard library made -- its signature
    is the standard library's, so nothing MODULE declares is called to put the question. The
    question runs MODULE's hook, and a hook can loop for ever on a repeated name, so it is
    bounded like a run: one that has not answered in QUESTION seconds is UNPROVEN, and the run it
    interrupted goes on."""
    probing[0] += 1
    restore = pristine()
    started = time.monotonic()
    remaining, interval = signal.setitimer(signal.ITIMER_REAL, QUESTION, 1)
    asking.append(busy[0])
    try:
        duplicate = ended(lambda: scanner(DUPLICATE, 0))
        readings = [ended(lambda one=one: scanner(one, 0)) for one in READINGS]
    except Forced:
        return "unproven"
    finally:
        asking.pop()
        signal.setitimer(signal.ITIMER_REAL, 0)
        late[0] = False
        if remaining:
            signal.setitimer(signal.ITIMER_REAL,
                             max(remaining - (time.monotonic() - started), 0.001), interval)
        restore()
        probing[0] -= 1
    if duplicate == "returned":
        return "unrefusing"
    return "refusing" if readings == ["returned", "returned"] else "unproven"


def ours(code):
    return code in mine or code in alias or os.path.abspath(code.co_filename) == target


def site():
    """The INNERMOST frame of MODULE's on the stack -- or, with none, what the probe is running: a
    scan made while MODULE runs is MODULE's, whichever thread it runs on."""
    frame = sys._getframe(1)
    while frame is not None:
        if ours(frame.f_code):
            return frame.f_code.co_qualname
        frame = frame.f_back
    return activity[0][0] if activity[0] is not None else "<probe>"


busy = [0]            # > 0 while the probe's own bookkeeping runs inside a call of MODULE's


def recorded(record):
    """RECORD's value, with a failure of the probe's own kept to be raised again at the end -- and a
    question's time, where it ran out while this was recording, ended as soon as this is done."""
    busy[0] += 1
    try:
        value = record()
    except BaseException as exc:
        faults.append(exc)
        raise
    finally:
        busy[0] -= 1
    if late[0] and asking and busy[0] == asking[-1]:
        late[0] = False
        raise Forced("the question went past %d seconds" % QUESTION)
    return value


pending = {}          # id of a frame of MODULE's a refusal is on its way out of -> that frame
read_from_environment = {}  # a variable MODULE's code read from `os.environ` -> the run that read it
real_environ_getitem = os._Environ.__getitem__


def environ_getitem(self, key):
    """A variable read while MODULE's code is on the stack -- or while the sweep is calling something
    MODULE only binds, which is MODULE's code running with none of MODULE's frames under it."""
    if self is os.environ and not probing[0] and activity[0] is not None and isinstance(key, str):
        frame = sys._getframe(1)
        while frame is not None and not ours(frame.f_code):
            frame = frame.f_back
        if frame is not None or not activity[0][0].startswith("<"):
            read_from_environment.setdefault(key, activity[0])
    return real_environ_getitem(self, key)


os._Environ.__getitem__ = environ_getitem


def refusing():
    """Hold every frame of MODULE's above a scan that refused: each must let the refusal out."""
    frames = []
    frame = sys._getframe(2)
    while frame is not None:
        if ours(frame.f_code):
            frames.append(frame)
        frame = frame.f_back
    if frames and activity[0] is not None and activity[0][0] == "<drive>":
        frames.pop()  # the program's entry point reports a refusal by what it returns
    if frames and not pending:
        M.set_events(TOOL, M.events.PY_RETURN | M.events.PY_YIELD | M.events.PY_UNWIND)
    for one in frames:
        pending[id(one)] = one


def let_out(code, offset, *rest):
    recorded(lambda: out_of(sys._getframe(3)))


def out_of(frame):
    if pending.get(id(frame)) is frame:
        del pending[id(frame)]
        if not pending:
            M.set_events(TOOL, 0)


def returned_over(code, offset, value):
    recorded(lambda: over(sys._getframe(3)))


def over(frame):
    if pending.get(id(frame)) is frame:
        del pending[id(frame)]
        swallowed.add(frame.f_code.co_qualname)
        if not pending:
            M.set_events(TOOL, 0)


callee = {}           # the name the sweep calls something by -> what it calls
ATOMS = (str, bytes, int, float, complex, bool, type(None))


def elsewhere(value):
    """Whether VALUE is not a value MODULE's code bound but a namespace of somebody else's: a
    module's dictionary, code, a frame, a class written outside MODULE, or the probe's own code."""
    if isinstance(value, (types.CodeType, types.FrameType)):
        return True
    if isinstance(value, dict):
        return any(value is vars(one) for one in list(sys.modules.values()) + [builtins])
    if isinstance(value, type):
        return value.__module__ != MODULE
    code = getattr(value, "__code__", None)
    return isinstance(code, types.CodeType) and os.path.abspath(code.co_filename) == probe_file


def holds(value, carriers):
    """Whether VALUE holds one of CARRIERS: is one, or reaches one through what it refers to --
    never through a function's code or its globals, which reach everything the module has."""
    wanted = {id(one) for one in carriers if one is not None}
    stack, seen, kept = [value], set(), []
    while stack:
        one = stack.pop()
        if id(one) in wanted:
            return True
        if isinstance(one, ATOMS) or id(one) in seen or isinstance(one, type) or elsewhere(one):
            continue
        seen.add(id(one))
        kept.append(one)
        if isinstance(one, types.FunctionType):
            stack += [one.__defaults__, one.__kwdefaults__, one.__closure__, one.__dict__]
        else:
            stack += gc.get_referents(one)
    return False


def defaults_of(function):
    """(key, value) for each default of FUNCTION: an index into `__defaults__`, or a keyword."""
    return (list(enumerate(function.__defaults__ or ()))
            + list((function.__kwdefaults__ or {}).items()))


held_by_default = {}  # (code, key of a default) -> (what the scanner refused with, the run)


def defaulted(where, carriers):
    """WHERE A CALLER COULD TAKE A REFUSAL AWAY. A default is what a caller gets by passing
    nothing, so a refusal a default holds is removed by any caller that passes that parameter --
    `options={}` for a default of `{"object_pairs_hook": one_reading}`, `hook=None`,
    `cls=json.JSONDecoder`. Every default of a function of MODULE's on the stack, and of the one
    the sweep is calling, that holds what the scanner refuses with is noted, and `taken_away`
    repeats the run with it taken out. A scan made while the module body runs cannot be repeated
    with a default changed -- running the body makes its functions again -- so there, and only
    there, the note is the answer."""
    functions, frame = [], sys._getframe(1)
    while frame is not None:
        if ours(frame.f_code):
            functions += [one for one in gc.get_referrers(frame.f_code)
                          if isinstance(one, types.FunctionType)]
        frame = frame.f_back
    called = callee.get(activity[0][0]) if activity[0] is not None else None
    if isinstance(called, types.FunctionType) and ours(called.__code__):
        functions.append(called)
    for function in functions:
        for key, value in defaults_of(function):
            if not holds(value, carriers):
                continue
            if activity[0] is None or activity[0][0] == "<module>":
                answers.setdefault(where, set()).add("unrefusing")
            else:
                code = alias.get(function.__code__, function.__code__)
                held_by_default.setdefault((code, key), (carriers, activity[0]))


def carriers_of(context):
    """What a scanner built over CONTEXT refuses with: the context, its pairs hook, its class where
    MODULE wrote that class, and what the hook itself calls through its closure or a `partial`. Read
    when the scanner is built, because that is when the scanner reads them."""
    kind = type(context)
    hook = getattr(context, "object_pairs_hook", None)
    inside = []
    for cell in getattr(hook, "__closure__", None) or ():
        try:
            inside.append(cell.cell_contents)
        except ValueError:
            pass
    if isinstance(hook, functools.partial):
        inside += [hook.func, *hook.args, *hook.keywords.values()]
    return tuple([context, hook, kind if kind.__module__ == MODULE else None]
                 + [one for one in inside if callable(one)])


def without(value, carriers):
    """VALUE copied with every one of CARRIERS taken out of it, at any depth: a decoder becomes
    the standard library's with no hook, a class MODULE wrote becomes the standard library's
    decoder class, and anything else -- a hook -- becomes `dict`, which is what a scanner with no
    hook builds an object with. A function holding one in its defaults or its closure is made
    again around the copies. The copy is `copy.deepcopy`'s, so what cannot be copied raises."""
    memo = {}
    for one in carriers:
        if one is not None and id(one) not in memo:
            memo[id(one)] = (NEUTRAL if isinstance(one, json.decoder.JSONDecoder)
                             else json.decoder.JSONDecoder if isinstance(one, type) else dict)
    stack, seen, found = [value], set(), []
    while stack:
        one = stack.pop()
        if isinstance(one, ATOMS) or id(one) in seen or isinstance(one, type) or elsewhere(one):
            continue
        seen.add(id(one))
        if isinstance(one, types.FunctionType):
            found.append(one)
            stack += [one.__defaults__, one.__kwdefaults__, one.__closure__, one.__dict__]
        else:
            stack += gc.get_referents(one)
    for function in reversed(found):
        if id(function) in memo or not holds(function, carriers):
            continue
        cells = []
        for cell in function.__closure__ or ():
            try:
                cells.append(types.CellType(copy.deepcopy(cell.cell_contents, memo)))
            except ValueError:
                cells.append(types.CellType())
        again = types.FunctionType(function.__code__, function.__globals__, function.__name__,
                                   copy.deepcopy(function.__defaults__, memo),
                                   tuple(cells) if function.__closure__ is not None else None)
        again.__kwdefaults__ = copy.deepcopy(function.__kwdefaults__, memo)
        again.__qualname__ = function.__qualname__
        again.__dict__.update(copy.deepcopy(function.__dict__, memo))
        memo[id(function)] = again
    return copy.deepcopy(value, memo)


def watch(inner, carriers):
    """INNER, a scanner the standard library made, with every scan through it answered for."""
    def scanning(string, idx, *rest, **named):
        if probing[0]:
            return inner(string, idx, *rest, **named)

        def record():
            where = site()
            answers.setdefault(where, set()).add(ask(inner))
            defaulted(where, carriers)
            observed[0] += 1
            return where, twice(string, idx)
        where, named_twice = recorded(record)
        try:
            value = inner(string, idx, *rest, **named)
        except Exception:
            if named_twice:
                recorded(refusing)
            raise
        if named_twice:
            answers[where].add("unrefusing")
        return value
    inner_of[scanning] = inner
    return scanning


def watching(real):
    def make_scanner(context):
        carriers = carriers_of(context)
        scanner = watch(real(context), carriers)
        if not probing[0]:
            def record():
                where = site()
                built.append((where, weakref.ref(scanner)))
                defaulted(where, carriers)
                observed[0] += 1
            recorded(record)
        return scanner
    return make_scanner


json.scanner.py_make_scanner = watching(real_py)
if real_c is not None:
    import _json
    json.scanner.c_make_scanner = watching(real_c)
    _json.make_scanner = json.scanner.c_make_scanner
json.scanner.make_scanner = json.scanner.c_make_scanner or json.scanner.py_make_scanner
for existing in gc.get_objects():
    if isinstance(existing, json.decoder.JSONDecoder) and existing is not reference:
        existing.scan_once = watch(existing.scan_once, carriers_of(existing))

# ---- what every code object of MODULE's must be seen to do ---------------------------------------
INERT = frozenset({
    "NOP", "RESUME", "CACHE", "EXTENDED_ARG", "NOT_TAKEN", "KW_NAMES", "POP_TOP", "COPY", "SWAP",
    "PUSH_NULL", "END_FOR", "END_SEND", "POP_ITER", "LOAD_CONST", "RETURN_CONST", "LOAD_SMALL_INT",
    "JUMP_FORWARD", "JUMP_BACKWARD", "JUMP_BACKWARD_NO_INTERRUPT", "JUMP", "JUMP_NO_INTERRUPT",
    "RETURN_VALUE", "RERAISE", "POP_EXCEPT", "PUSH_EXC_INFO", "STORE_FAST", "DELETE_FAST",
    "STORE_DEREF", "DELETE_DEREF", "STORE_NAME", "DELETE_NAME", "STORE_GLOBAL", "DELETE_GLOBAL",
    "MAKE_CELL", "COPY_FREE_VARS", "LOAD_CLOSURE", "RETURN_GENERATOR",
})
STOPS = frozenset({"RETURN_VALUE", "RETURN_CONST", "RAISE_VARARGS", "RERAISE", "JUMP_FORWARD",
                   "JUMP_BACKWARD", "JUMP_BACKWARD_NO_INTERRUPT", "JUMP", "JUMP_NO_INTERRUPT"})
CALLS = frozenset({"CALL", "CALL_FUNCTION_EX", "CALL_KW"})
FLIP = {"POP_JUMP_IF_FALSE": "POP_JUMP_IF_TRUE", "POP_JUMP_IF_TRUE": "POP_JUMP_IF_FALSE",
        "POP_JUMP_IF_NONE": "POP_JUMP_IF_NOT_NONE", "POP_JUMP_IF_NOT_NONE": "POP_JUMP_IF_NONE"}
JUMPS = frozenset(dis.hasjrel) | frozenset(dis.hasjabs) | frozenset(getattr(dis, "hasjump", ()))
listing = {code: list(dis.get_instructions(code)) for code in tree}
table = {code: dis.Bytecode(code).exception_entries for code in tree}
position = {code: {one.offset: index for index, one in enumerate(listing[code])} for code in tree}
ending = {}           # (code, offset of a call) -> whether anything it was seen to call can return
needed = {}


def cannot_return(called):
    if called in (sys.exit, os._exit, os.abort) or type(called).__name__ == "Quitter":
        return True
    code = getattr(getattr(called, "__func__", called), "__code__", None)
    if not isinstance(code, types.CodeType) or code.co_flags & (
            inspect.CO_GENERATOR | inspect.CO_COROUTINE | inspect.CO_ASYNC_GENERATOR):
        return False
    return not any(one.opname in ("RETURN_VALUE", "RETURN_CONST")
                   for one in dis.get_instructions(code))


def reachable(code, roots):
    ins, at = listing[code], position[code]
    seen, stack = set(), list(roots)
    while stack:
        offset = stack.pop()
        if offset in seen or offset not in at:
            continue
        seen.add(offset)
        index = at[offset]
        one = ins[index]
        if one.opname in CALLS and ending.get((code, offset)) is False:
            continue
        if one.opname not in STOPS and index + 1 < len(ins):
            stack.append(ins[index + 1].offset)
        if one.opcode in JUMPS:
            stack.append(one.argval)
    return seen


def required(code, handlers=True):
    """The instructions of CODE that must run: every one reachable from its entry -- and from each of
    its handlers -- that could run code or read a value other than a constant."""
    if (code, handlers) not in needed:
        roots = [listing[code][0].offset]
        if handlers:
            roots += [entry.target for entry in table[code]]
        offsets = reachable(code, roots)
        needed[(code, handlers)] = frozenset(one.offset for one in listing[code]
                                             if one.offset in offsets and one.opname not in INERT)
    return needed[(code, handlers)]


def conditional(code):
    ins = listing[code]
    return [(one.offset, frozenset((ins[index + 1].offset, one.argval)))
            for index, one in enumerate(ins)
            if one.opname in FLIP and not (index and ins[index - 1].opname in ("CHECK_EXC_MATCH",
                                                                               "CHECK_EG_MATCH"))]


branches = {code: conditional(code) for code in tree}
both_ways = {(code, offset): way for code in tree for offset, way in branches[code]}
ran = {code: set() for code in tree}
arcs = {code: set() for code in tree}
natural = {code: set() for code in tree}  # the arcs taken while nothing the probe altered was running
first = {}            # (code, offset) -> the (label, replay) that first ran that instruction
entered = {}          # code -> the (label, replay) that first ran any of it
first_branch = {}     # (code, offset) -> the (label, replay) that first took that branch
armed = []            # [code, offset, exception] raised where it is due, once
forcing = [0]         # > 0 while a run the probe altered is running
M = sys.monitoring
TOOL = next(tool for tool in range(6) if M.get_tool(tool) is None)
M.use_tool_id(TOOL, "decode-probe")
EVENTS = M.events.INSTRUCTION | M.events.BRANCH | M.events.CALL


class Forced(BaseException):
    """What the probe raises into a handler naming no class it can read, or to end a run that has
    gone on longer than any run of a parser should: a branch turned, or a document handed to a
    function that was never written for it, can loop for ever."""


SECONDS = 10


def expired(signum, frame):
    # Never inside the probe's own bookkeeping: the timer fires again a second later, and a run that
    # is still going is by then in MODULE's code. Inside the question it does -- at the depth of
    # bookkeeping the question started at, which is MODULE's hook running -- and `ask` answers for a
    # question that did not end.
    if asking and busy[0] > asking[-1]:
        late[0] = True
    elif asking or (not probing[0] and not busy[0]):
        raise Forced("a run went past %d seconds" % SECONDS)


signal.signal(signal.SIGALRM, expired)


def instruction(code, offset):
    def record():
        nonlocal code
        code = alias.get(code, code)
        ran[code].add(offset)
        if (code, offset) not in first and activity[0] is not None:
            first[(code, offset)] = activity[0]
            entered.setdefault(code, activity[0])
        if not forcing[0] or probing[0]:
            return None
        for one in armed:
            if one[0] is code and one[1] == offset:
                armed.remove(one)
                return one[2](sys._getframe(3))
        return None
    due = recorded(record)
    if due is not None:
        raise due
    return None if forcing[0] else M.DISABLE


def branch(code, source, destination):
    return recorded(lambda: took(code, source, destination))


def took(code, source, destination):
    code = alias.get(code, code)
    arcs[code].add((source, destination))
    if not forcing[0]:
        natural[code].add((source, destination))
    if (code, source) not in first_branch and activity[0] is not None:
        first_branch[(code, source)] = activity[0]
    way = both_ways.get((code, source))
    if forcing[0] or way is None:
        return None
    return M.DISABLE if way <= {one for start, one in arcs[code] if start == source} else None


def calling(code, offset, called, first_argument):
    return recorded(lambda: seen_calling(code, offset, called))


def seen_calling(code, offset, called):
    code = alias.get(code, code)
    if ending.get((code, offset)):
        return None if forcing[0] else M.DISABLE
    returns = not cannot_return(called)
    if returns or (code, offset) not in ending:
        ending[(code, offset)] = returns
        for key in [one for one in needed if one[0] is code]:
            del needed[key]
    return None


M.register_callback(TOOL, M.events.INSTRUCTION, instruction)
M.register_callback(TOOL, M.events.BRANCH, branch)
M.register_callback(TOOL, M.events.CALL, calling)
M.register_callback(TOOL, M.events.PY_RETURN, returned_over)
M.register_callback(TOOL, M.events.PY_YIELD, returned_over)
M.register_callback(TOOL, M.events.PY_UNWIND, let_out)
for code in tree:
    M.set_local_events(TOOL, code, EVENTS)


def current(code):
    return installed.get(code, code)


def install(code, replacement, undo):
    """Run REPLACEMENT wherever MODULE's CODE would run, and remake what makes it, until UNDO."""
    alias[replacement] = code
    M.set_local_events(TOOL, replacement, EVENTS)
    undo.append((code, installed.get(code)))
    installed[code] = replacement
    for function in [one for one in gc.get_objects() if isinstance(one, types.FunctionType)
                     and (one.__code__ is code or alias.get(one.__code__) is code)]:
        undo.append((function, function.__code__))
        function.__code__ = replacement
    above = parent.get(code)
    if above is not None:
        remade = current(above).replace(co_consts=tuple(
            replacement if (const is code or alias.get(const) is code) else const
            for const in current(above).co_consts))
        install(above, remade, undo)


def uninstall(undo):
    while undo:
        holder, previous = undo.pop()
        if isinstance(holder, str):
            if previous is None:
                os.environ.pop(holder, None)
            else:
                os.environ[holder] = previous
        elif isinstance(holder, types.FunctionType) and isinstance(previous, tuple):
            holder.__defaults__, holder.__kwdefaults__ = previous
        elif isinstance(holder, types.FunctionType):
            holder.__code__ = previous
        elif previous is None:
            installed.pop(holder, None)
        else:
            installed[holder] = previous


def turned(code, offset):
    raw = bytearray(current(code).co_code)
    raw[offset] = opcode.opmap[FLIP[opcode.opname[raw[offset]]]]
    return current(code).replace(co_code=bytes(raw))


# ---- the runs ------------------------------------------------------------------------------------
class Sink:
    def __init__(self):
        self.buffer = self

    def write(self, data):
        return len(data)

    def flush(self):
        pass

    def close(self):
        pass


def perform(label, replay):
    saved = activity[0], sys.stdout
    activity[0], sys.stdout = (label, replay), Sink()
    signal.setitimer(signal.ITIMER_REAL, SECONDS, 1)
    try:
        return replay()
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        activity[0], sys.stdout = saved


spec = importlib.util.spec_from_file_location(MODULE, target)
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
os.chdir(scratch)


def reimported():
    try:
        exec(current(top), {"__name__": spec.name, "__file__": target, "__builtins__": builtins})
    except (Exception, SystemExit, Forced) as exc:
        return "raised:" + type(exc).__name__
    return "returned"


activity[0] = ("<module>", reimported)
signal.setitimer(signal.ITIMER_REAL, SECONDS, 1)
exec(top, module.__dict__)
signal.setitimer(signal.ITIMER_REAL, 0)
activity[0] = None


def driving(index, one):
    head, _, document = one.rpartition("|")
    argv = (head or "review").split() + ["--out", "driven%d" % index]
    if document:
        argv.append(os.path.abspath(document))

    def replay():
        try:
            return "returned:%r" % (module.main(argv),)
        except (Exception, SystemExit, Forced) as exc:
            return "raised:" + type(exc).__name__
    return replay


driven = []
if callable(getattr(module, "main", None)):
    for index, one in enumerate(drives):
        driven.append(perform("<drive>", driving(index, one)))
drive = ",".join(driven) or "absent"


def members(where, value):
    """(name, value) for everything VALUE refers to. A callable is CALLED rather than opened, so
    of a callable only its own attributes are members; of anything else, whatever the garbage
    collector says it refers to -- named by key, index or attribute where VALUE names it, and by
    type where not. A decoder's own machinery, where it is the standard library's, is asked at
    its scanner instead."""
    if isinstance(value, type):
        return [("%s.%s" % (value.__qualname__, key), one) for key, one in vars(value).items()]
    found = []
    if isinstance(value, dict):
        found += [("%s[%s]" % (where, str(key)[:24]), one) for key, one in value.items()]
        found += [("%s.keys()[%d]" % (where, index), key) for index, key in enumerate(value)]
    elif isinstance(value, (list, tuple)):
        found += [("%s[%d]" % (where, index), one) for index, one in enumerate(value)]
    attributes = getattr(value, "__dict__", None)
    if isinstance(attributes, dict) and not elsewhere(attributes):
        found += [("%s.%s" % (where, key), one) for key, one in attributes.items()
                  if not (isinstance(value, json.decoder.JSONDecoder) and key in PLAIN
                          and (key == "scan_once" or one is PLAIN[key]
                               or (type(one) is type(PLAIN[key]) and one == PLAIN[key])))]
    if not callable(value):
        named = {id(one) for _, one in found} | {id(attributes)}
        found += [("%s<%s>" % (where, type(one).__name__), one) for one in gc.get_referents(value)
                  if id(one) not in named]
    return found


def held(namespace):
    """(name, callable) for EVERY CALLABLE MODULE'S NAMESPACE REACHES, and not a list of the
    places a callable may sit: a name, a class body, a container of any kind at any depth, an
    attribute, a key. Whatever is reached is taken as it is -- nothing here asks whether it
    decodes, or whose code it runs. A function of MODULE's is named by its qualified name, and
    anything else by its path."""
    found, seen, kept = [], set(), []
    stack = list(reversed(list(namespace.items())))
    while stack:
        where, value = stack.pop()
        if isinstance(value, (staticmethod, classmethod)):
            value = value.__func__
        if isinstance(value, ATOMS) or id(value) in seen or elsewhere(value):
            continue
        seen.add(id(value))
        kept.append(value)
        if isinstance(value, types.FunctionType) and value.__code__ in mine:
            where = value.__qualname__
        if callable(value) and not isinstance(value, type):
            found.append((where, value))
        stack.extend(reversed(members(where, value)))
    return found


def arguments(signature, document):
    positional, keywords = [], {}
    for parameter in signature.parameters.values():
        if parameter.default is not parameter.empty:
            continue
        if parameter.kind is parameter.KEYWORD_ONLY:
            keywords[parameter.name] = document
        elif parameter.kind in (parameter.POSITIONAL_ONLY, parameter.POSITIONAL_OR_KEYWORD):
            positional.append(document)
    signature.bind(*positional, **keywords)
    return positional, keywords


def run(function, positional, keywords, document, depth=0):
    result = function(*positional, **keywords)
    values = [result]
    if inspect.iscoroutine(result):
        import asyncio
        values = [asyncio.run(result)]
    elif inspect.isasyncgen(result):
        import asyncio

        async def drained(generator):
            return [one async for one in generator]
        values = asyncio.run(drained(result))
    elif inspect.isgenerator(result):
        values = list(result)
    for value in values:
        if depth < 3 and callable(value) and not isinstance(value, type):
            try:
                signature = inspect.signature(value)
            except (TypeError, ValueError):
                continue
            run(value, *arguments(signature, document), document, depth + 1)


def as_path():
    handle, path = tempfile.mkstemp(dir=scratch, suffix=".json")
    with os.fdopen(handle, "w", encoding="utf-8") as written:
        written.write(DUPLICATE)
    return path


skipped, swept, called = [], [], set()


def sweep():
    progress = False
    for name, function in held(vars(module)):
        code = getattr(function, "__code__", None)
        own = code is not None and code in mine
        if id(function) in called or (own and required(code, False) <= ran[code]):
            continue
        called.add(id(function))
        callee[name] = function
        progress = True
        try:
            signature = inspect.signature(function)
        except (TypeError, ValueError):
            skipped.append(name)
            continue
        attempts, before, reached = [], observed[0], {code} if own else set()
        for document in (lambda: DUPLICATE, lambda: READINGS[1], as_path):
            text = document()
            positional, keywords = arguments(signature, text)
            sizes = {one: len(ran[one]) for one in tree}

            def replay(function=function, positional=positional, keywords=keywords, text=text):
                try:
                    run(function, positional, keywords, text)
                    return "returned"
                except (Exception, SystemExit, Forced) as exc:
                    return "raised:" + type(exc).__name__
            attempts.append(perform(name, replay))
            reached |= {one for one in tree if len(ran[one]) != sizes[one]}
            if own and all(required(one, False) <= ran[one] for one in reached):
                break
            if not own and observed[0] > before:
                break
        swept.append("%s:%s" % (name, "/".join(attempts)))
    return progress


def clauses(code, handler):
    """(the first instruction of each clause, what its test loads) for the handler at HANDLER; a
    handler naming no class the probe can read is one clause with no test."""
    ins, at = listing[code], position[code]
    index = at[handler]
    if ins[index].opname != "PUSH_EXC_INFO":
        return [(handler, None)]
    found = []
    index += 1
    while index < len(ins):
        start = index
        while index < len(ins) and ins[index].opname in ("LOAD_GLOBAL", "LOAD_NAME", "LOAD_FAST",
                                                       "LOAD_FAST_CHECK", "LOAD_DEREF",
                                                       "LOAD_ATTR", "BUILD_TUPLE"):
            index += 1
        if index + 2 >= len(ins) or ins[index].opname != "CHECK_EXC_MATCH" or index == start:
            found.append((ins[start].offset, None))
            break
        found.append((ins[index + 2].offset, ins[start:index]))
        if ins[index + 1].opname != "POP_JUMP_IF_FALSE" or ins[index + 1].argval not in at:
            break
        index = at[ins[index + 1].argval]
    return found


def raising(test):
    def build(frame):
        stack = []
        try:
            for one in test or ():
                if one.opname in ("LOAD_GLOBAL", "LOAD_NAME"):
                    stack.append(frame.f_globals[one.argval] if one.argval in frame.f_globals
                                 else getattr(builtins, one.argval))
                elif one.opname == "LOAD_ATTR":
                    stack.append(getattr(stack.pop(), one.argval))
                elif one.opname == "BUILD_TUPLE":
                    stack[len(stack) - one.argval:] = [tuple(stack[len(stack) - one.argval:])]
                else:
                    stack.append(frame.f_locals[one.argval])
        except Exception:
            stack = []
        caught = stack[-1] if stack else None
        caught = caught[0] if isinstance(caught, tuple) and caught else caught
        if not (isinstance(caught, type) and issubclass(caught, BaseException)):
            return Forced("a handler the probe's documents never entered")
        try:
            return caught()
        except Exception:
            return caught.__new__(caught)
    return build


forced, tried = [], set()


def attempt(key, label, base, alteration):
    """BASE again, with ALTERATION in force for the whole of it."""
    tried.add(key)

    def replay():
        undo = []
        forcing[0] += 1
        M.restart_events()
        try:
            recorded(lambda: alteration(undo))
            return base()
        finally:
            if forcing[0] == 1:
                armed.clear()
            recorded(lambda: uninstall(undo))
            forcing[0] -= 1
    perform(label, replay)
    forced.append(key)


def alter():
    progress = False
    for code in tree:
        for handler in sorted({entry.target for entry in table[code]}):
            if not (required(code) & reachable(code, [handler])) - ran[code]:
                continue
            protected = sorted({one.offset for entry in table[code] if entry.target == handler
                                for one in listing[code] if entry.start <= one.offset < entry.end})
            reached = [offset for offset in protected if (code, offset) in first]
            for body, test in clauses(code, handler):
                key = ("raise", code, handler, body)
                if body in ran[code] or key in tried or not reached:
                    continue
                label, base = first[(code, reached[0])]
                injection = [code, reached[0], raising(test)]
                attempt(key, label, base, lambda undo, injection=injection: armed.append(injection))
                progress = True
        for offset, way in branches[code]:
            taken = {one for start, one in arcs[code] if start == offset}
            if not taken or (code, offset) not in first_branch:
                continue
            for destination in sorted(way - taken):
                key = ("turn", code, offset, destination)
                if key in tried:
                    continue
                label, base = first_branch[(code, offset)]
                attempt(key, label, base,
                        lambda undo, code=code, offset=offset: install(code, turned(code, offset), undo))
                progress = True
    return progress


def exhausted(code):
    return required(code) <= ran[code]


def together():
    """AND EVERY BRANCH OF A CODE OBJECT STILL SHORT, TURNED AT ONCE. One branch turned alone can
    leave a state a second one needs: `if strict: decoder = Strict()` then `if strict: return
    decoder.decode(text)`, the second turned alone, fails on a name the first never bound. So a code
    object the turns above did not exhaust is run once more with every branch it only ever took one
    way turned together, in a repeat of the first run that entered it."""
    progress = False
    for code in tree:
        key = ("together", code)
        if key in tried or exhausted(code) or code not in entered:
            continue
        single = [offset for offset, way in branches[code]
                  if len({one for start, one in natural[code] if start == offset}) == 1]
        if len(single) < 2:
            continue
        label, base = entered[code]

        def alteration(undo, code=code, single=single):
            for offset in single:
                install(code, turned(code, offset), undo)
        attempt(key, label, base, alteration)
        progress = True
    return progress


def constants(code):
    for const in code.co_consts:
        if isinstance(const, str):
            yield const
        elif isinstance(const, (tuple, frozenset)):
            yield from (one for one in const if isinstance(one, str))


strings = sorted({one for code in tree for one in constants(code) if len(one) <= 64 and "\n" not in one})


def environment():
    """AND THE ENVIRONMENT IS AN INPUT TOO. A variable MODULE's code reads is set, in a repeat of the
    run that first read it, to every string MODULE's code holds as a constant: `if os.environ.get(
    "UPSTROKE_LOOSE")` is a branch and is turned above, but `HOOKS.get(os.environ.get("HOOK",
    "strict"))` takes no branch at all, and the value that selects the other entry is one of the
    file's own strings."""
    progress = False
    for key, (label, base) in list(read_from_environment.items()):
        for value in strings:
            k = ("environment", key, value)
            if k in tried:
                continue

            def alteration(undo, key=key, value=value):
                undo.append((key, os.environ.get(key)))
                os.environ[key] = value
            attempt(k, label, base, alteration)
            progress = True
    return progress


def redefault(code, key, value, undo):
    """Every function made from MODULE's CODE, with the default at KEY set to VALUE until UNDO."""
    for function in [one for one in gc.get_objects() if isinstance(one, types.FunctionType)
                     and alias.get(one.__code__, one.__code__) is code]:
        positional, named = function.__defaults__, function.__kwdefaults__
        if isinstance(key, int) and positional is not None and key < len(positional):
            undo.append((function, (positional, named)))
            function.__defaults__ = (positional[:key] + (value(positional[key]),)
                                     + positional[key + 1:])
        elif isinstance(key, str) and named is not None and key in named:
            undo.append((function, (positional, named)))
            function.__kwdefaults__ = dict(named, **{key: value(named[key])})


def taken_away():
    """A REFUSAL A DEFAULT HOLDS IS A REFUSAL ITS CALLER CAN TAKE AWAY, so the run that saw one is
    repeated with that default copied without what the scanner refused with -- and what the decode
    does then is the answer, not the note. A default that cannot be copied cannot be asked, and
    leaves its function unproven."""
    progress = False
    for (code, key), (carriers, (label, base)) in list(held_by_default.items()):
        k = ("taken", code, key)
        if k in tried:
            continue

        def alteration(undo, code=code, key=key, carriers=carriers):
            try:
                redefault(code, key, lambda value: without(value, carriers), undo)
            except Exception:
                answers.setdefault(code.co_qualname, set()).add("unproven")
        attempt(k, label, base, alteration)
        progress = True
    return progress


def scalars(code):
    for const in code.co_consts:
        if type(const) in (str, bytes, int, float, bool):
            yield const
        elif isinstance(const, (tuple, frozenset)):
            yield from (one for one in const if type(one) in (str, bytes, int, float, bool))


kinds = sorted({(type(one).__name__, one) for code in tree for one in scalars(code)
                if not isinstance(one, (str, bytes)) or (len(one) <= 64 and "\n" not in str(one))})


def selected():
    """AND A DEFAULT IS AN INPUT, as the environment is. A default a run used can select a decode
    without holding one -- `HOOKS[mode]` with `mode="strict"`, `(None, one_reading)[index]` -- so
    it is set, in a repeat of the run that first entered its function, to each value OF ITS OWN
    KIND that MODULE's code holds as a constant, and to the kind's own empty value, asked of the
    kind: a string default to each of the file's strings and `""`, a number to each of its
    numbers and `0`, a flag to both."""
    progress, made = False, {}
    for one in gc.get_objects():
        if isinstance(one, types.FunctionType) and (one.__defaults__ or one.__kwdefaults__):
            made.setdefault(alias.get(one.__code__, one.__code__), one)
    for code in tree:
        if code not in entered or entered[code][0] == "<module>" or code not in made:
            continue
        label, base = entered[code]
        for key, value in defaults_of(made[code]):
            if type(value) not in (str, bytes, int, float, bool):
                continue
            own = {(type(value).__name__, type(value)()), ("bool", True), ("bool", False)}
            for kind, other in sorted(set(kinds) | own):
                k = ("selected", code, key, kind, other)
                if type(value).__name__ != kind or other == value or k in tried:
                    continue
                attempt(k, label, base, lambda undo, code=code, key=key, other=other:
                        redefault(code, key, lambda _: other, undo))
                progress = True
    return progress


while alter() or sweep() or together() or environment() or taken_away() or selected():
    pass

gc.collect()
for where, scanner in built:
    alive = scanner()
    if alive is not None:
        answers.setdefault(where, set()).add(ask(inner_of[alive]))
M.set_events(TOOL, 0)
for code in list(tree) + list(alias):
    M.set_local_events(TOOL, code, 0)
M.free_tool_id(TOOL)

if faults:
    raise faults[0]


def listed(items):
    return ",".join(sorted(items)) or "-"


sites = {"refusing": set(), "unrefusing": set(), "unproven": set()}
for where, given in answers.items():
    for answer in given:
        sites[answer].add(where)
sites["unproven"] |= {code.co_qualname for code in tree if not exhausted(code)}
sites["unrefusing"] |= swallowed
with open(report_file, "w", encoding="utf-8") as report:
    report.write("decoded=%s unrefusing=%s unproven=%s skipped=%s\n" % (
        "yes" if answers else "no", listed(sites["unrefusing"]), listed(sites["unproven"]),
        listed(skipped)))
    report.write("refusing=%s drive=%s swept=%s forced=%d\n" % (
        listed(sites["refusing"]), drive, listed(swept), len(forced)))
DECODEPROBE
# `PYTHONDONTWRITEBYTECODE=1` as well as the `sys.dont_write_bytecode` inside, because the probe is
# also a script somebody runs by hand: importing the parser must not leave a `__pycache__` under
# `scripts/`, and a required gate that writes into the tree it judges is the shape this whole
# section exists to keep out. `TMPDIR` keeps the probe's own scratch inside the trap above, and the
# probe runs MODULE from inside that scratch, so a file MODULE writes to a relative path lands
# there too. The probe's stdout and stderr go to a log and its report to a file of its own, so
# nothing the module prints can reach an assertion; no report means the probe itself failed, which
# no assertion below accepts.
decode_probe() {  # decode_probe MODULE DRIVE...: the report's two lines as one, or why none
  local module="$1" status=0
  shift
  rm -f "$tmp/decode-report"
  TMPDIR="$tmp" PYTHONDONTWRITEBYTECODE=1 "$parser_python" "$tmp/decode-probe.py" "$module" \
    "$tmp/decode-report" "$@" > "$tmp/decode-probe.log" 2>&1 || status=$?
  if ((status != 0)) || [[ ! -s "$tmp/decode-report" ]]; then
    printf 'no report: the probe exited %s: %s' "$status" "$(tail -n 1 "$tmp/decode-probe.log")"
    return 0
  fi
  printf '%s | %s' "$(sed -n 1p "$tmp/decode-report")" "$(sed -n 2p "$tmp/decode-report")"
}
printf '{"verdict":"PASS","findings":[]}\n' > "$tmp/probe-drive.json"
# THE PROBE IS ITSELF UNDER TEST, because a probe that has stopped watching agrees with a clean
# parser and says so in the same words. So it runs first over thirty-three stand-ins whose answers
# are known, and MUTATIONS OF THE PROBE WERE WATCHED AGAINST THEM -- each the smallest text change
# that undoes one rule, and each run through this whole file:
#
#   mutation of the probe                              what moved
#   a raise on the duplicate read as a refusal,        `unrelated`
#     readings ignored (round 2's rule)
#   the first reading alone asked                      `unrelated`
#   the last reading alone asked                       `unrelated`
#   the unproven left out of the verdict line          `bare`, `exhausted`, `settling`, `taken`,
#                                                        `unnamed`, `unreadable`, `unrelated`
#   `py_make_scanner` not replaced                     `scanner`, `swallowed`
#   `c_make_scanner` left as the standard library's    `scanner`
#     while `make_scanner` and `_json.make_scanner`
#     are still watched
#   `_json.make_scanner` not replaced                  `scanner`
#   the scanners of decoders that already exist left   sixteen
#     unwrapped
#   a scanner MODULE built and still holds never asked `taken`, `unused`
#   a refusal not followed out of the frames above     `delivered`, `swallowed`
#     the scan
#   the drive's entry point held to that rule too      `delivered`, and the parser itself
#   only the swept callable's own frame held, and      `delivered`, `swallowed`
#     only under the sweep (round 5's rule)
#   the document actually scanned not judged           `nested`
#   a scan with no frame of MODULE's under it          `exhausted`, `held`, `reached`, `routes`,
#     answered for by nobody (round 5's rule)            `threaded`, `unnamed`
#   only the code of callables MODULE holds held to    `exhausted`
#     coverage, not every code object
#   no instruction required to run                     `bare`, `branches`, `exhausted`, `settling`,
#                                                        `unnamed`, `unreadable`
#   handler code not required to run                   `contracts`, `delivered`, `handlers`,
#                                                        `routes`, `swallowed`
#   a handler no run entered left unentered            the same five, and the parser itself
#   a branch taken one way only left unturned          ten, and the parser itself
#   a code object's branches never turned together     `branches`
#   a variable MODULE reads never set to the file's    `branches`, `contracts`, `delivered`,
#     own strings                                        `swallowed`
#   a call of something that cannot return not         `after-exit`
#     ending its block
#   the question put to the code as the probe altered  `pristine`, `settling`, `unrelated`
#     it
#   the sweep skipping every function anything has     `branches`, `exhausted`
#     entered (round 5's rule)
#   the sweep stopping once the swept function has     `swallowed`, `unrelated`
#     run, whatever it called
#   nothing held but MODULE's own functions            `exhausted`, `held`, `reached`, `routes`,
#                                                        `unnamed`, `wrapped`
#   what a dictionary, a list or a tuple holds, and    `exhausted`, `held`, `unnamed`
#     what anything else refers to, not walked
#   what a call returns not called                     `taken`, `unnamed`
#   keyword-only parameters left out of the call       `contracts`, `exhausted`, `unused`
#   a coroutine called and not awaited                 `contracts`
#   the sweep's later documents never tried            fourteen
#   the report written to stdout (round 2's channel)   every stand-in, and the parser itself
#   the skipped left out of the verdict line           `unreadable`
#   the drive removed                                  every stand-in, and the parser itself
#   only the first drive run                           the parser itself
#   a sweep that calls nothing                         twenty-five
#   a hardcoded clean verdict line                     twenty-six
#   a class body held only for the functions MODULE    `held`, `unnamed`
#     wrote (round 6's rule)
#   a dictionary's keys not walked                     `held`
#   what a value refers to beyond its items and its    `held`, `unnamed`
#     attributes not walked
#   a callable's own attributes not walked             `held`
#   a class written outside MODULE walked as MODULE's  every stand-in, and the parser itself
#   the probe's own code walked and called             `taken`
#   a decoder's own machinery walked and called        `taken`, `unused`
#   a refusal a default holds never taken away         `taken`
#     (round 6's rule)
#   the defaults of what the sweep is calling not      `taken`
#     looked at
#   what the hook calls not among what the scanner     `taken`
#     refuses with
#   the class MODULE wrote for it not among them       `taken`
#   the decoder not among them                         `taken`
#   a function holding one not made again around the   `taken`
#     copy
#   a scan the module body makes not answered where    `taken`
#     a default holds what it refuses with
#   a default that cannot be copied not unproven       `taken`
#   a default never set to the file's values (round    `exhausted`, `scanner`, `selected`
#     6's rule)
#   a kind's own empty value never tried               `selected`
#   the question held outside the bound (round 6's     the probe never ends on `settling`, and the
#     rule)                                              gate was killed at 400 seconds
#
# and none of the fifty-four that ran to their end moved an assertion of any other family in this
# file. One more was measured and moves no assertion, because what it changes is a time: a question
# whose time runs out while the probe is recording is ended when the recording is done, and without
# that `settling` took 6, 12 and 13 seconds in three runs where it takes 4, as the timer fired again
# and again inside the probe's own callback. The ten-second bound on a run is not in the table: it
# is how the probe ends a run that would otherwise never end, and no stand-in here runs long enough
# to reach it.
#
# Each stand-in decodes THE WAY THE PARSER DOES -- out of a block object its caller built, not out
# of a string handed to `read` -- so the drive is load-bearing: the sweep cannot construct a block,
# and without the drive `hooked` and `bare` go silent.
probe_stand_in() {  # probe_stand_in <name> <decode-expression>, and the rest of the module on stdin
  { printf 'import json\n\n\nclass Block:\n    def __init__(self, content):\n'
    printf '        self.content = content\n\n\ndef one_reading(pairs):\n    seen = set()\n'
    printf '    for name, _ in pairs:\n        if name in seen:\n'
    printf '            raise ValueError("names %%r twice" %% name)\n        seen.add(name)\n'
    printf '    return dict(pairs)\n\n\ndef read(text, block):\n    return %s\n' "$2"
    printf '\n\ndef main(argv):\n    read("", Block(open(argv[-1]).read()))\n    return 0\n'
    cat
  } > "$tmp/probe-$1.py"
}
probe_expect() {  # probe_expect <stand-in> <report> [<drive document>]
  expect "MUT-JSON-REPEATED-NAME-CHOSEN [$1]" \
    "$(decode_probe "$tmp/probe-$1.py" "${3:-$tmp/probe-drive.json}")" "$2"
}
# The control: the decode the program takes, hooked, which the pair must clear.
probe_stand_in hooked 'json.loads(block.content, object_pairs_hook=one_reading)' < /dev/null
probe_expect hooked \
  'decoded=yes unrefusing=- unproven=- skipped=- | refusing=read drive=returned:0 swept=- forced=0'
# The same program with the hook off: THE DEFECT ITSELF, on the path the program takes. With no
# hook nothing runs `one_reading`, so it is swept as well -- and a hook handed a document where a
# list of pairs belongs raises on every document the sweep has, so its code never runs past the
# first line: unproven.
probe_stand_in bare 'json.loads(block.content)' < /dev/null
probe_expect bare \
  'decoded=yes unrefusing=read unproven=one_reading skipped=- | refusing=- drive=returned:0 swept=one_reading:raised:ValueError/raised:ValueError/raised:ValueError forced=0'
# A SECOND DECODER THAT GENUINELY REFUSES IS NOT THIS DEFECT. It is safe -- run on the pair it
# raises on the duplicate and reads both readings, the answer the decode the parser takes gives --
# and round 0's count of one turned red on it. Green, and named among the refusing.
probe_stand_in second-hooked 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


def second_reader(text):
    return json.loads(text, object_pairs_hook=one_reading)
PYSHAPE
probe_expect second-hooked \
  'decoded=yes unrefusing=- unproven=- skipped=- | refusing=read,second_reader drive=returned:0 swept=second_reader:raised:ValueError forced=0'
# THE TWO SPELLINGS THAT WALKED THROUGH ROUND 1, which both spell `object_pairs_hook=one_reading`
# and neither of which hooks anything: the identifier bound to a parameter, and the keyword handed
# to a decoder class that drops it. Run rather than read, both return on the duplicate.
probe_stand_in disguised 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


class IgnoringHook(json.JSONDecoder):
    def __init__(self, **kwargs):
        super().__init__()


def by_shadowed_hook(text, one_reading=None):
    return json.loads(text, object_pairs_hook=one_reading)


def by_decoder_override(text):
    return json.loads(text, object_pairs_hook=one_reading, **{"cls": IgnoringHook})
PYSHAPE
probe_expect disguised \
  'decoded=yes unrefusing=by_decoder_override,by_shadowed_hook unproven=- skipped=- | refusing=read drive=returned:0 swept=IgnoringHook.__init__:raised:TypeError/raised:TypeError/raised:TypeError,by_decoder_override:returned,by_shadowed_hook:returned forced=0'
# EVERY ROUTE ROUND 1 HAD TO DISCLOSE AS OUT OF ITS REACH, in one module: the two it named as
# imports, the rebinding, the `getattr`, `json.load` on a handle, `JSONDecoder().decode`, and the
# `ns.json.loads` its own review found escaping. `imported` is among the answers without being a
# function of this module's at all: `from json import loads as imported` BINDS a decoder to a
# module-level name, so the binding is swept and answered for like anything else the module holds.
# `rebound` is the same object under a second name and is answered once, by identity.
probe_stand_in routes 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import json as shortened
import types
from json import loads as imported

rebound = json.loads
namespace = types.SimpleNamespace(json=json)


def by_alias(text):
    return shortened.loads(text)


def by_binding(text):
    return rebound(text)


def by_decoder(text):
    return json.JSONDecoder().decode(text)


def by_getattr(text):
    return getattr(json, "loads")(text)


def by_handle(path):
    with open(path) as handle:
        return json.load(handle)


def by_import(text):
    return imported(text)


def by_namespace(text):
    return namespace.json.loads(text)
PYSHAPE
probe_expect routes \
  'decoded=yes unrefusing=by_alias,by_binding,by_decoder,by_getattr,by_handle,by_import,by_namespace,imported unproven=- skipped=- | refusing=read drive=returned:0 swept=by_alias:returned,by_binding:returned,by_decoder:returned,by_getattr:returned,by_handle:raised:FileNotFoundError/raised:FileNotFoundError/returned,by_import:returned,by_namespace:returned,imported:returned forced=2'
# AN EXCEPTION IS NOT A REFUSAL -- round 2's false green, which both of its reviews found. A hook
# demanding a finding id, and an `object_hook` indexing one, raised on round 2's probe document
# only because its finding had no id; given one, both return on the duplicate. And the two
# decoders here that DO raise on the duplicate raise on a reading as well -- one refuses any
# severity, one any empty findings array -- so neither is shown to refuse the repetition: UNPROVEN,
# and not acceptable. They are why both readings are asked: each is cleared by one reading alone.
probe_stand_in unrelated 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


def schema_hook(pairs):
    obj = dict(pairs)
    if "severity" in obj and "id" not in obj:
        raise ValueError("a finding needs an id")
    return obj


def finding_objects(obj):
    if "severity" in obj:
        return {"id": obj["id"], "severity": obj["severity"]}
    return obj


def refuses_severities(pairs):
    if any(name == "severity" for name, _ in pairs):
        raise ValueError("no severity may be written here")
    return dict(pairs)


def refuses_empty_findings(pairs):
    obj = dict(pairs)
    if obj.get("findings") == []:
        raise ValueError("a findings array may not be empty")
    return obj


def by_schema_hook(text):
    return json.loads(text, object_pairs_hook=schema_hook)


def by_finding_hook(text):
    return json.loads(text, object_hook=finding_objects)


def by_severity_refusal(text):
    return json.loads(text, object_pairs_hook=refuses_severities)


def by_empty_refusal(text):
    return json.loads(text, object_pairs_hook=refuses_empty_findings)
PYSHAPE
probe_expect unrelated \
  'decoded=yes unrefusing=by_finding_hook,by_schema_hook unproven=by_empty_refusal,by_severity_refusal skipped=- | refusing=read drive=returned:0 swept=by_empty_refusal:raised:ValueError,by_finding_hook:returned,by_schema_hook:returned/returned/raised:JSONDecodeError,by_severity_refusal:raised:ValueError,finding_objects:raised:TypeError/returned/returned,refuses_empty_findings:raised:ValueError/raised:ValueError/raised:ValueError,refuses_severities:raised:ValueError/raised:ValueError/raised:ValueError,schema_hook:raised:ValueError/raised:ValueError/raised:ValueError forced=1'
# THE DECODES THAT ESCAPED ROUND 2'S REPLACEMENT: `raw_decode`, which its copied decoder class
# inherited unwatched, and `json.decoder.JSONDecoder`, which its copy of the module never replaced
# -- with that class imported by name, and a subclass of it.
probe_stand_in escapes 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import json.decoder
from json.decoder import JSONDecoder as Imported


class Subclassed(json.JSONDecoder):
    pass


def by_raw_decode(text):
    return json.JSONDecoder().raw_decode(text)[0]


def by_decoder_module(text):
    return json.decoder.JSONDecoder().decode(text)


def by_imported_class(text):
    return Imported().decode(text)


def by_subclass(text):
    return Subclassed().decode(text)
PYSHAPE
probe_expect escapes \
  'decoded=yes unrefusing=by_decoder_module,by_imported_class,by_raw_decode,by_subclass unproven=- skipped=- | refusing=read drive=returned:0 swept=by_decoder_module:returned,by_imported_class:returned,by_raw_decode:returned,by_subclass:returned forced=0'
# AND THE DECODES THAT GO STRAIGHT TO A SCANNER. `raw_decode` is where a decoder scans, but nothing
# obliges anything to go through it: `scan_once` is reachable on the decoder, `json.scanner` builds
# a scanner out of anything, and a subclass may replace `raw_decode` with something that never
# calls the one it replaced. Round 3 watched `raw_decode` alone and had to disclose all three; round
# 5 watched construction and the scanner a decoder is built with, and a scanner built any other way
# -- over the default decoder `json.loads` itself uses, or over a context object that is no decoder
# at all -- was built and scanned with nobody watching. THE SCANNER IS WHERE THE QUESTION IS PUT
# NOW: every name the standard library builds a scanner under is replaced, and the default
# decoder's scanner is wrapped where it stands. Seven of the eight below scan unhooked through a
# watched scanner and are red -- `by_own_raw_decode`'s scan is answered for as `OwnScan.raw_decode`,
# the frame of MODULE's that made it -- and `by_hooked_scanner` is the same route hooked, which
# genuinely refuses and stays green.
probe_stand_in scanner 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import _json
import json.scanner


class OwnScan(json.JSONDecoder):
    def raw_decode(self, s, idx=0):
        return self.scan_once(s, idx)


class Context:
    strict = True
    object_hook = None
    object_pairs_hook = None
    parse_float = float
    parse_int = int
    parse_constant = float
    memo = {}


def by_scan_once(text):
    return json.JSONDecoder().scan_once(text, 0)[0]


def by_made_scanner(text):
    return json.scanner.make_scanner(json.JSONDecoder())(text, 0)[0]


def by_own_raw_decode(text):
    return OwnScan().raw_decode(text)[0]


def by_hooked_scanner(text):
    return json.JSONDecoder(object_pairs_hook=one_reading).scan_once(text, 0)[0]


def by_default_scanner(text):
    return json._default_decoder.scan_once(text, 0)[0]


def by_c_scanner(text):
    return json.scanner.c_make_scanner(json.JSONDecoder())(text, 0)[0]


def by_python_scanner(text):
    return json.scanner.py_make_scanner(json._default_decoder)(text, 0)[0]


def by_context_scanner(text):
    return _json.make_scanner(Context())(text, 0)[0]
PYSHAPE
probe_expect scanner \
  'decoded=yes unrefusing=OwnScan.raw_decode,by_c_scanner,by_context_scanner,by_default_scanner,by_made_scanner,by_python_scanner,by_scan_once unproven=- skipped=- | refusing=by_hooked_scanner,read drive=returned:0 swept=OwnScan.raw_decode:raised:AttributeError/raised:AttributeError/raised:AttributeError,by_c_scanner:returned,by_context_scanner:returned,by_default_scanner:returned,by_hooked_scanner:raised:ValueError/returned,by_made_scanner:returned,by_own_raw_decode:returned,by_python_scanner:returned,by_scan_once:returned forced=1'
# AND ONE THAT IS NEVER SCANNED WITH on any run the documents make: built at module level and read
# from only behind a branch nothing takes. Its scanner is asked where it was BUILT, because MODULE
# still holds it at the end, and named for the module body that built it; and the branch that reads
# it is turned, so `by_branch` scans with it too and is named for that.
probe_stand_in unused 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


_DECODER = json.JSONDecoder()


def by_branch(text, *, mode):
    if mode == "loose":
        return _DECODER.decode(text)
    return None
PYSHAPE
probe_expect unused \
  'decoded=yes unrefusing=<module>,by_branch unproven=- skipped=- | refusing=read drive=returned:0 swept=by_branch:returned/returned/returned forced=1'
# WHAT A CALL THAT REACHES NO SCANNER IS CONCLUDED FROM -- rounds 4 and 5, one in each direction.
# Round 4 concluded nothing and reported green, so `by_branch_reader` (handed the probe's document
# as `mode`, taking neither branch) and `by_bytes_reader` (raising on a `str` before it decodes)
# both passed. Round 5 asked every line to run, which reds both -- and reds a helper nothing calls
# whose untaken line is `return 0`. Per instruction and per branch separates them: the branch in
# `by_branch_reader` is TURNED, so its decode runs and answers; the call in `by_bytes_reader` never
# runs on anything the probe has, so it is UNPROVEN; and `by_unconsumed` is why every code object is
# asked on its own -- the generator expression is a code object of its own on the same line as the
# statement that builds it, and it never runs.
#
# THE GREEN SHAPES ARE IN THE SAME MODULE, because a rule that reds everything separates nothing.
# `by_whole_body` encodes and never decodes. `by_reached_branch` takes both of its branches on the
# probe's own documents. `by_short_name` runs its one line and returns, and the table it indexes is
# where round 5 had to stop: `readers["loose"]` is a bare `json.loads` the default argument never
# selects. It is held now -- a callable inside a module-level container is held like one bound to a
# name -- so the table's decoder is swept and answered for, and it is red. And so is `by_short_name`
# itself, which round 6 had to leave: its default is an input now, set to each of the file's
# strings, and under `name="loose"` it decodes unhooked.
probe_stand_in exhausted 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


readers = {"named": lambda text: None, "loose": json.loads}


def by_branch_reader(text, *, mode):
    if mode == "loose":
        return json.loads(text)
    return None


def by_bytes_reader(data):
    return json.loads(data.decode("utf-8"))


def by_whole_body(value):
    return json.dumps(value)


def by_reached_branch(text):
    if text.startswith("{"):
        return json.loads(text, object_pairs_hook=one_reading)
    return None


def by_short_name(text, name="named"):
    return readers[name](text)


def by_unconsumed(text):
    rows = (json.loads(text) for _ in [1])
    return rows is None
PYSHAPE
probe_expect exhausted \
  'decoded=yes unrefusing=by_branch_reader,by_short_name,readers[loose] unproven=by_bytes_reader,by_unconsumed.<locals>.<genexpr> skipped=- | refusing=by_reached_branch,read drive=returned:0 swept=by_branch_reader:returned/returned/returned,by_bytes_reader:raised:AttributeError/raised:AttributeError/raised:AttributeError,by_reached_branch:raised:ValueError,by_short_name:returned,by_unconsumed:returned,by_whole_body:returned,readers[loose]:returned forced=9'
# THE DECODE BEHIND SOMEBODY ELSE'S FRAME -- round 3's false green, and the reason the whole stack
# is searched. `functools.singledispatch` puts one frame of `functools` between the caller and the
# decode; round 3 stepped over the `json` frames, found `functools`, and answered for nobody. Here
# it is on the path the program itself takes, so the drive alone reaches it.
cat > "$tmp/probe-wrapped.py" <<'PYSHAPE'
import json
from functools import singledispatch


class Block:
    def __init__(self, content):
        self.content = content


def read(text, block):
    return singledispatch(json.loads)(block.content)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0
PYSHAPE
probe_expect wrapped \
  'decoded=yes unrefusing=read unproven=- skipped=- | refusing=- drive=returned:0 swept=singledispatch:returned/returned/returned forced=0'
# AND WHAT THAT COSTS, MEASURED RATHER THAN ASSERTED AWAY. A scan with any frame of MODULE's under
# it is MODULE's, so a LIBRARY the parser calls that decodes for its own reasons is answered for as
# the parser and turns it red. That is the closed side of a question with no third answer: the open
# side is the case above, where one frame of `functools` hid the parser's own decode. The library's
# own code is another file's and is not held to coverage. `scripts/pr-review-parse.py` imports
# `collections`, `json`, `os`, `re`, `sys` and `tempfile` and calls nothing that decodes, so today
# this costs it nothing.
cat > "$tmp/probe-borrowed-lib.py" <<'PYSHAPE'
import json


def settings():
    return json.loads('{"a":1,"a":2}')
PYSHAPE
cat > "$tmp/probe-borrowed.py" <<'PYSHAPE'
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import probe_borrowed_lib


class Block:
    def __init__(self, content):
        self.content = content


def one_reading(pairs):
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise ValueError("names %r twice" % name)
        seen.add(name)
    return dict(pairs)


def read(text, block):
    probe_borrowed_lib.settings()
    return json.loads(block.content, object_pairs_hook=one_reading)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0
PYSHAPE
cp "$tmp/probe-borrowed-lib.py" "$tmp/probe_borrowed_lib.py"
probe_expect borrowed \
  'decoded=yes unrefusing=read unproven=- skipped=- | refusing=read drive=returned:0 swept=- forced=0'
# EVERY FUNCTION CALLED BY ITS REAL CONTRACT: the keyword-only parameter round 2 left out of the
# call, a positional-only one, a method and a static method in a class body, a lambda bound at
# module level, and a coroutine and a generator, neither of which runs any of its body when called.
probe_stand_in contracts 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


class Reader:
    def by_method(self, text):
        return json.loads(text)

    @staticmethod
    def by_staticmethod(text):
        return json.loads(text)


by_lambda = lambda text: json.loads(text)


def by_keyword_only(*, text):
    return json.loads(text)


def by_positional_only(text, /):
    return json.loads(text)


async def by_coroutine(text):
    return json.loads(text)


def by_generator(text):
    yield json.loads(text)
PYSHAPE
probe_expect contracts \
  'decoded=yes unrefusing=<lambda>,Reader.by_method,Reader.by_staticmethod,by_coroutine,by_generator,by_keyword_only,by_positional_only unproven=- skipped=- | refusing=read drive=returned:0 swept=<lambda>:returned,Reader.by_method:returned,Reader.by_staticmethod:returned,by_coroutine:returned,by_generator:returned,by_keyword_only:returned,by_positional_only:returned forced=7'
# AND A READER THE BYTECODE DOES NOT NAME. Round 3 chose what to sweep from `co_names`, which holds
# the names a function LOOKS UP -- and a name the caller supplies is never looked up. A reader
# taking the module as a defaulted parameter decodes through it, its `co_names` is `('loads',)`, and
# the whole function fell out of the set. Nothing chooses now: every callable nothing has run is
# called. Both readers here decode through a parameter; one is handed the module and one a bound
# `loads`.
probe_stand_in defaulted 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


def by_defaulted_module(text, json=json):
    return json.loads(text)


def by_defaulted_loads(text, loads=json.loads):
    return loads(text)
PYSHAPE
probe_expect defaulted \
  'decoded=yes unrefusing=by_defaulted_loads,by_defaulted_module unproven=- skipped=- | refusing=read drive=returned:0 swept=by_defaulted_loads:returned,by_defaulted_module:returned forced=0'
# AND THE ROUTES NO FILTER REACHED -- round 4's false green, and the reason there is no filter.
# `setattr(holder, "json", json)` binds the module to an attribute, which no `ast.Name` of round
# 4's fixed point carries and no `co_names` of round 3's carries either, so `holder.json.loads`
# was swept by neither; `functools.singledispatch` around a bound `loads` is the same escape one
# call deeper, on a helper nothing calls. Both are ordinary ways to write a reader, so both come
# with the same shape written correctly: a hooked reader reached through the SAME attribute, and
# a hooked one reached through the same wrapper. The last four are the shape with no function of
# this module's in it at all: a decoder bound straight to a module-level name, and one wrapped in
# a `functools.partial` -- neither is a function of the file's, and both decode.
# `bound_strict` is `strict_read` under a second name and is answered once, by identity, which is
# why it has no row of its own. Four red, four green, out of one rule -- every callable nothing has
# run is called, and what it does decides it.
probe_stand_in reached 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import functools
from types import SimpleNamespace

holder = SimpleNamespace()


def strict_read(text):
    return json.loads(text, object_pairs_hook=one_reading)


setattr(holder, "json", json)
setattr(holder, "strict", strict_read)


def by_attribute(text):
    return holder.json.loads(text)


def by_strict_attribute(text):
    return holder.strict(text)


def by_dispatch(text):
    return functools.singledispatch(json.loads)(text)


def by_strict_dispatch(text):
    return functools.singledispatch(strict_read)(text)


bound_loose = json.loads
bound_strict = strict_read
partial_loose = functools.partial(json.loads)
partial_strict = functools.partial(strict_read)
PYSHAPE
probe_expect reached \
  'decoded=yes unrefusing=bound_loose,by_attribute,by_dispatch,partial_loose unproven=- skipped=- | refusing=read,strict_read drive=returned:0 swept=bound_loose:returned,by_attribute:returned,by_dispatch:returned,by_strict_attribute:raised:ValueError,by_strict_dispatch:raised:ValueError,partial_loose:returned,partial_strict:raised:ValueError,strict_read:raised:ValueError forced=0'
# A REFUSAL REACHED AND THROWN AWAY, and a decoder that is not the one that decoded. `Forgiving`'s
# scanner refuses the repetition, and its own `decode` catches the refusal and hands back
# `{"verdict": "PASS", "findings": []}`, which is the finding this whole section exists for, one
# level above the scanner. `ScanForgiving` does the same through `scan_once` -- round 5 counted a
# refusal only where `raw_decode` raised, so both of its reviews passed it -- and `by_converted`
# lets the refusal out of the decoder as a `RuntimeError` and returns over THAT. Every frame of
# MODULE's above a scan that refused must let the refusal out, so all three are red, and so is
# `by_loose_forgiving`, which forgives only behind an environment branch the probe turns.
# `by_rebuilt` decodes through an UNHOOKED decoder's scanner and then calls `JSONDecoder.__init__`
# on that same object again WITH a hook, and it is answered for where its scanner RAN. Each comes
# with the same thing done properly: `Reporting` catches the refusal and RAISES, which is how a
# parser reports it, `ScanStrict` drives `scan_once` and lets the refusal out, `by_kept` builds its
# decoder hooked and leaves it alone, and `by_late` builds unhooked, hooks it, and rebuilds its
# scanner before anything is decoded -- which genuinely refuses, and is green.
probe_stand_in swallowed 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import json.scanner
import os


class Forgiving(json.JSONDecoder):
    def __init__(self):
        super().__init__(object_pairs_hook=one_reading)

    def decode(self, text):
        try:
            return super().decode(text)
        except ValueError:
            return {"verdict": "PASS", "findings": []}


class ScanForgiving(json.JSONDecoder):
    def __init__(self):
        super().__init__(object_pairs_hook=one_reading)

    def decode(self, text):
        try:
            return self.scan_once(text, 0)[0]
        except ValueError:
            return {"verdict": "PASS", "findings": []}


class ScanStrict(json.JSONDecoder):
    def __init__(self):
        super().__init__(object_pairs_hook=one_reading)

    def decode(self, text):
        return self.scan_once(text, 0)[0]


class Reporting(json.JSONDecoder):
    def __init__(self):
        super().__init__(object_pairs_hook=one_reading)

    def decode(self, text):
        try:
            return super().decode(text)
        except ValueError as exc:
            raise RuntimeError("the review names a key twice") from exc


def by_forgiving(text):
    return Forgiving().decode(text)


def by_scan_forgiving(text):
    return ScanForgiving().decode(text)


def by_scan_strict(text):
    return ScanStrict().decode(text)


def by_reporting(text):
    return Reporting().decode(text)


def by_converted(text):
    try:
        return by_reporting(text)
    except RuntimeError:
        return {"verdict": "PASS", "findings": []}


def by_loose_forgiving(text):
    if os.environ.get("UPSTROKE_LOOSE"):
        return Forgiving().decode(text)
    return Reporting().decode(text)


def by_rebuilt(text):
    decoder = json.JSONDecoder()
    try:
        return decoder.scan_once(text, 0)[0]
    finally:
        json.JSONDecoder.__init__(decoder, object_pairs_hook=one_reading)


def by_kept(text):
    return json.JSONDecoder(object_pairs_hook=one_reading).decode(text)


class Late(json.JSONDecoder):
    def __init__(self):
        super().__init__()
        self.object_pairs_hook = one_reading
        self.scan_once = json.scanner.py_make_scanner(self)


def by_late(text):
    return Late().decode(text)
PYSHAPE
probe_expect swallowed \
  'decoded=yes unrefusing=Forgiving.decode,ScanForgiving.decode,by_converted,by_forgiving,by_loose_forgiving,by_rebuilt,by_scan_forgiving unproven=- skipped=- | refusing=Forgiving.decode,Reporting.decode,ScanForgiving.decode,ScanStrict.decode,by_kept,by_late,read drive=returned:0 swept=Forgiving.__init__:raised:TypeError/raised:TypeError/raised:TypeError,Forgiving.decode:raised:TypeError/raised:TypeError/raised:TypeError,Late.__init__:raised:TypeError/raised:TypeError/raised:TypeError,Reporting.__init__:raised:TypeError/raised:TypeError/raised:TypeError,Reporting.decode:raised:TypeError/raised:TypeError/raised:TypeError,ScanForgiving.__init__:raised:TypeError/raised:TypeError/raised:TypeError,ScanForgiving.decode:raised:AttributeError/raised:AttributeError/raised:AttributeError,ScanStrict.__init__:raised:TypeError/raised:TypeError/raised:TypeError,ScanStrict.decode:raised:AttributeError/raised:AttributeError/raised:AttributeError,by_converted:returned,by_forgiving:returned,by_kept:raised:ValueError,by_late:raised:ValueError,by_loose_forgiving:raised:RuntimeError/returned/raised:RuntimeError,by_rebuilt:returned,by_reporting:raised:RuntimeError,by_scan_forgiving:returned/returned,by_scan_strict:raised:ValueError/returned forced=16'
# AND A REFUSAL THE PATH THE PROGRAM TAKES RETURNS OVER. The rule above is not the sweep's: the
# drive runs `main`, `main` runs `read`, and `read` forgives the repeated name behind an environment
# branch no document takes. The branch is turned, `read` returns over the refusal, and it is red.
# `main` returns 1 over the same refusal on the run that does not forgive, and that is how a program
# reports one -- the entry point a drive calls is the one frame the rule leaves to the program.
cat > "$tmp/probe-delivered.py" <<'PYSHAPE'
import json
import os


class Block:
    def __init__(self, content):
        self.content = content


def one_reading(pairs):
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise ValueError("names %r twice" % name)
        seen.add(name)
    return dict(pairs)


def read(text, block):
    try:
        return json.loads(block.content, object_pairs_hook=one_reading)
    except ValueError:
        if os.environ.get("UPSTROKE_LOOSE"):
            return {"verdict": "PASS", "findings": []}
        raise


def main(argv):
    try:
        read("", Block(open(argv[-1]).read()))
    except ValueError:
        return 1
    return 0
PYSHAPE
printf '{"verdict":"PASS","findings":[{"id":"CRITICAL","severity":"P1"}],"findings":[]}\n' \
  > "$tmp/probe-drive-twice.json"
probe_expect delivered \
  'decoded=yes unrefusing=read unproven=- skipped=- | refusing=read drive=returned:1 swept=- forced=10' "$tmp/probe-drive-twice.json"
# AND A DECODER WHOSE OWN `decode` THE PROBE MUST NOT CALL -- round 3's false red. Round 3 put its
# question through `decoder.decode`, a method a module declares however it likes; this one requires
# a keyword, round 3's call omitted it, and a decoder that DOES refuse the repetition was reported
# unproven and blocked. The question is put to the scanner the standard library made, whose
# signature the standard library fixes, so nothing of the module's is called to put it. Green.
probe_stand_in strict-keyword 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


class StrictDecoder(json.JSONDecoder):
    def __init__(self):
        super().__init__(object_pairs_hook=one_reading)

    def decode(self, text, *, review):
        return super().decode(text)


def by_strict_decoder(text):
    return StrictDecoder().decode(text, review=True)
PYSHAPE
probe_expect strict-keyword \
  'decoded=yes unrefusing=- unproven=- skipped=- | refusing=StrictDecoder.decode,read drive=returned:0 swept=StrictDecoder.__init__:raised:TypeError/raised:TypeError/raised:TypeError,StrictDecoder.decode:raised:TypeError/raised:TypeError/raised:TypeError,by_strict_decoder:raised:ValueError forced=0'
# AND ONE THE PROBE CANNOT CALL BY ITS SIGNATURE, because the signature cannot be read: it is not
# called with a guess, it is reported -- and a skipped function is red, like the code in it that
# nothing ran.
probe_stand_in unreadable 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


def by_unreadable_signature(text):
    return json.loads(text)


by_unreadable_signature.__signature__ = "not a signature"
PYSHAPE
probe_expect unreadable \
  'decoded=yes unrefusing=- unproven=by_unreadable_signature skipped=by_unreadable_signature | refusing=read drive=returned:0 swept=- forced=0'
# NOTHING THE MODULE PRINTS REACHES THE ANSWER. Round 2 read its report off stdout, and a helper
# printing JSON turned a correct parser red. Both helpers here are called and both print -- one of
# them a counterfeit report line -- and the report is still the one the probe wrote.
probe_stand_in encoder-only 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import os
import sys


def print_review(value):
    print(json.dumps(value))


def announce(value):
    sys.stdout.write(json.dumps({"verdict": "PASS"}) + "\n")
    os.write(1, b"decoded=yes unrefusing=- unproven=- skipped=-\n")
PYSHAPE
probe_expect encoder-only \
  'decoded=yes unrefusing=- unproven=- skipped=- | refusing=read drive=returned:0 swept=announce:returned,print_review:returned forced=0'
# AND THE SHAPE ROUND 1 REFUSED: the decode reached through a factory and an attribute chain, still
# hooked, still refusing -- green, and named by the method that made it.
cat > "$tmp/probe-chained.py" <<'PYSHAPE'
import json


class Block:
    def __init__(self, content):
        self.content = content


def one_reading(pairs):
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise ValueError("names %r twice" % name)
        seen.add(name)
    return dict(pairs)


class Strict:
    def loads(self, text):
        return json.loads(text, object_pairs_hook=one_reading)


class Codec:
    json = Strict()


def codec():
    return Codec()


def read(text, block):
    return codec().json.loads (block.content)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0
PYSHAPE
probe_expect chained \
  'decoded=yes unrefusing=- unproven=- skipped=- | refusing=Strict.loads drive=returned:0 swept=- forced=0'
# ROUND 5'S OWN FALSE GREEN, IN THE FUNCTIONS THE PROGRAM RUNS. Round 5 skipped every function a
# drive entered, so the branch a drive does not take was taken by nobody. Every reader here is
# entered by `main` on the probe's document and every one takes its hooked path there: the branch
# round 5's review put at the top of `parse_prose_review`, reached only by a document starting `[`
# here; the helper its other review routed the parser's decode through, unhooked under
# `UPSTROKE_LOOSE`; that helper as one conditional expression; and the hook itself chosen by a
# conditional whose other arm is a constant. Each branch is turned and each decode runs unhooked:
# four red. `by_environment_value` takes no branch at all -- the variable's value is a key into a
# table whose other entry is no hook -- so the run that read the variable is repeated with it set to
# each of the file's own strings, `"loose"` among them, and it is red too. `by_either_hook` turns to
# the same hook and stays green; `by_correlated` binds its decoder under one branch and uses it
# under a second, so turning either alone fails on a name the other never bound, and it is run once
# more with both turned and is green; and `finding_count` is round 5's false red -- a helper whose
# untaken branch is `return 0` -- which ran both ways and is green.
cat > "$tmp/probe-branches.py" <<'PYSHAPE'
import json
import os


def one_reading(pairs):
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise ValueError("names %r twice" % name)
        seen.add(name)
    return dict(pairs)


def by_startswith(content):
    if content.startswith("["):
        return json.loads(content, cls=json.JSONDecoder)
    return json.loads(content, object_pairs_hook=one_reading)


def by_environment(content):
    if os.environ.get("UPSTROKE_LOOSE"):
        return json.loads(content)
    return json.loads(content, object_pairs_hook=one_reading)


def by_one_line(content):
    return json.loads(content) if os.environ.get("UPSTROKE_LOOSE") else json.loads(content, object_pairs_hook=one_reading)


def by_constant_hook(content):
    hook = None if os.environ.get("UPSTROKE_LOOSE") else one_reading
    return json.loads(content, object_pairs_hook=hook)


HOOKS = {"strict": one_reading, "loose": None}


def by_environment_value(content):
    return json.loads(content, object_pairs_hook=HOOKS.get(os.environ.get("UPSTROKE_HOOK", "strict")))


def by_either_hook(content):
    hook = one_reading if os.environ.get("UPSTROKE_STRICT") else one_reading
    return json.loads(content, object_pairs_hook=hook)


class StrictDecoder(json.JSONDecoder):
    def __init__(self):
        super().__init__(object_pairs_hook=one_reading)


def by_correlated(content):
    if os.environ.get("UPSTROKE_STRICT_CLASS"):
        decoder = StrictDecoder()
    if os.environ.get("UPSTROKE_STRICT_CLASS"):
        return decoder.decode(content)
    return json.loads(content, object_pairs_hook=one_reading)


def finding_count(findings):
    if not findings:
        return 0
    return len(findings)


def main(argv):
    content = open(argv[-1]).read()
    by_startswith(content)
    by_one_line(content)
    by_constant_hook(content)
    by_environment_value(content)
    by_either_hook(content)
    by_correlated(content)
    return finding_count(by_environment(content)["findings"])
PYSHAPE
probe_expect branches \
  'decoded=yes unrefusing=by_constant_hook,by_environment,by_environment_value,by_one_line,by_startswith unproven=- skipped=- | refusing=by_constant_hook,by_correlated,by_either_hook,by_environment,by_environment_value,by_one_line,by_startswith drive=returned:0 swept=by_correlated:raised:ValueError/returned/raised:JSONDecodeError forced=57'
# WHAT REACHES A DECODE WITHOUT BEING A NAMED ATTRIBUTE AT IMPORT TIME -- round 5's review found the
# first. A module `__getattr__` returning `json.loads` is called, and it RETURNS the decoder: what a
# call hands back that can be called is called too. A metaclass `__call__` runs when its class is
# called, and it is a function in a class body of MODULE's like any method. A function held only in
# a list, with its name deleted, is no attribute at all, and `TABLE["loose"]` is a decoder in a
# dictionary: a module-level container is walked for callables. And a property nothing reads is
# code in the file that never runs, which is unproven. `HookedMeta` is the metaclass done properly,
# and green. The rest of the list -- an import alias, a `functools.partial`, a scanner made in C --
# is `routes`, `reached` and `scanner` above.
probe_stand_in unnamed 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


def __getattr__(name):
    return json.loads


class DecodingMeta(type):
    def __call__(cls, text):
        return json.loads(text)


class ByMetaclass(metaclass=DecodingMeta):
    pass


class HookedMeta(type):
    def __call__(cls, text):
        return json.loads(text, object_pairs_hook=one_reading)


class ByHookedMetaclass(metaclass=HookedMeta):
    pass


def _listed(text):
    return json.loads(text)


READERS = [_listed]
del _listed
TABLE = {"loose": json.loads}


class Review:
    text = ""

    @property
    def verdict(self):
        return json.loads(self.text)
PYSHAPE
probe_expect unnamed \
  'decoded=yes unrefusing=DecodingMeta.__call__,TABLE[loose],__getattr__,_listed unproven=Review.verdict skipped=- | refusing=HookedMeta.__call__,read drive=returned:0 swept=DecodingMeta.__call__:returned,HookedMeta.__call__:raised:ValueError,Review.verdict:raised:AttributeError/raised:AttributeError/raised:AttributeError,TABLE[loose]:returned,__getattr__:returned,_listed:returned forced=0'
# A HANDLER NO DOCUMENT ENTERS. `by_handler` decodes only when `text.upper()` raises, and nothing
# the probe hands it makes that happen; the handler is entered by raising what it catches where its
# body runs, and it decodes unhooked. `by_raising_handler` is entered by the refusal itself, raises,
# and is green.
probe_stand_in handlers 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


def by_handler(text):
    try:
        return text.upper()
    except AttributeError:
        return json.loads(text)


def by_raising_handler(text):
    try:
        return json.loads(text, object_pairs_hook=one_reading)
    except ValueError as exc:
        raise RuntimeError("the review names a key twice") from exc
PYSHAPE
probe_expect handlers \
  'decoded=yes unrefusing=by_handler unproven=- skipped=- | refusing=by_raising_handler,read drive=returned:0 swept=by_handler:returned,by_raising_handler:raised:RuntimeError forced=2'
# THE DOCUMENT THE PROGRAM ACTUALLY SCANNED, not only the probe's. `findings_once` refuses a
# repeated `findings` and nothing else, so the pair -- which repeats `findings` -- reads it as
# refusing; the drive's document repeats a finding's `witness`, the scan returns on it, and that is
# red whatever the pair said.
cat > "$tmp/probe-nested.py" <<'PYSHAPE'
import json


class Block:
    def __init__(self, content):
        self.content = content


def findings_once(pairs):
    if [name for name, _ in pairs].count("findings") > 1:
        raise ValueError("names 'findings' twice")
    return dict(pairs)


def read(text, block):
    return json.loads(block.content, object_pairs_hook=findings_once)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0
PYSHAPE
printf '{"verdict":"PASS","findings":[{"id":"CRITICAL","severity":"P1","witness":"a test","witness":null}]}\n' \
  > "$tmp/probe-drive-nested.json"
probe_expect nested \
  'decoded=yes unrefusing=read unproven=- skipped=- | refusing=read drive=returned:0 swept=- forced=0' "$tmp/probe-drive-nested.json"
# A BRANCH TURNED INSIDE A HOOK IS NOT THE HOOK THE MODULE HAS. This hook refuses an object of more
# than a thousand names before it looks for a repeated one; no document has one, so the branch is
# turned -- and while it is turned, the hook raises on everything. The question is put to the hook
# as MODULE wrote it, so the scanner still refuses, and this correct parser is green.
cat > "$tmp/probe-pristine.py" <<'PYSHAPE'
import json


class Block:
    def __init__(self, content):
        self.content = content


def one_reading(pairs):
    if len(pairs) > 1000:
        raise ValueError("an object of more than a thousand names")
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise ValueError("names %r twice" % name)
        seen.add(name)
    return dict(pairs)


def read(text, block):
    return json.loads(block.content, object_pairs_hook=one_reading)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0
PYSHAPE
probe_expect pristine \
  'decoded=yes unrefusing=- unproven=- skipped=- | refusing=read drive=returned:0 swept=- forced=1'
# A SCAN WITH NO FRAME OF MODULE'S UNDER IT. `read` starts a thread whose target is the standard
# library's own `decode`, so the stack the scan runs on holds nothing of the parser's -- round 5
# answered for nobody and reported the thread clean. A scan made while MODULE runs is MODULE's: it
# is answered for as the run the probe was making, `<drive>`.
cat > "$tmp/probe-threaded.py" <<'PYSHAPE'
import json
import threading


class Block:
    def __init__(self, content):
        self.content = content


def one_reading(pairs):
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise ValueError("names %r twice" % name)
        seen.add(name)
    return dict(pairs)


def read(text, block):
    worker = threading.Thread(target=json.JSONDecoder().decode, args=(block.content,))
    worker.start()
    worker.join()
    return json.loads(block.content, object_pairs_hook=one_reading)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0
PYSHAPE
probe_expect threaded \
  'decoded=yes unrefusing=<drive> unproven=- skipped=- | refusing=read drive=returned:0 swept=- forced=0'
# CODE AFTER A CALL THAT CANNOT RETURN IS NOT CODE THAT CAN RUN. Python compiles a statement that
# follows a `try` into each way out of it, so a helper written below the parser's own entry block --
# `if __name__ == "__main__":` around a `try` whose body is `sys.exit(main(sys.argv[1:]))` -- is
# made by a copy of the module body placed after the `sys.exit`, and nothing ever runs that copy. It
# is not asked for, because what the call was seen to call cannot return; asked for, it would red
# every helper appended to the parser below its entry point, `print(json.dumps(value))` round 2's
# among them. Green.
cat > "$tmp/probe-after-exit.py" <<'PYSHAPE'
import json
import sys


class Block:
    def __init__(self, content):
        self.content = content


def one_reading(pairs):
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise ValueError("names %r twice" % name)
        seen.add(name)
    return dict(pairs)


def read(text, block):
    return json.loads(block.content, object_pairs_hook=one_reading)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except SystemExit:
        raise


def later(value):
    return json.dumps(value)
PYSHAPE
probe_expect after-exit \
  'decoded=yes unrefusing=- unproven=- skipped=- | refusing=read drive=returned:0 swept=later:returned forced=1'
# ROUND 6'S FIRST ESCAPE, AND WHY NO PLACE A CALLABLE CAN SIT IS NAMED ANY MORE. Round 6 held what a
# module-level name, a class body and a module-level container held, and a class body only for the
# functions MODULE wrote -- so `read = staticmethod(json.loads)` in a class was never called, and
# the gate passed a reader that returns `{'verdict': 'PASS', 'findings': []}` from a document naming
# `findings` twice. Every list of places is the next escape, so what is swept now is whatever the
# module's namespaces refer to, by the garbage collector's own account: the class-bound reader, a
# `partial` in a class inside a class, a function's attribute, a dictionary's key, and a reader
# three containers deep in a `deque` are each red, and nothing here names a class body, an
# attribute, a key or a `deque`. `strict_read`, reached through a class body as a `staticmethod`,
# genuinely refuses.
probe_stand_in held 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import collections
import functools


def strict_read(text):
    return json.loads(text, object_pairs_hook=one_reading)


class ExtraReviewReaders:
    read = staticmethod(json.loads)
    strict = staticmethod(strict_read)

    class Nested:
        loose = functools.partial(json.loads)


strict_read.fallback = functools.partial(json.loads)
QUEUED = collections.deque([{"deep": [(functools.partial(json.loads),)]}])
KEYED = {functools.partial(json.loads): "loose"}
PYSHAPE
probe_expect held \
  'decoded=yes unrefusing=ExtraReviewReaders.Nested.loose,ExtraReviewReaders.read,KEYED.keys()[0],QUEUED<dict>[deep][0][0],strict_read.fallback unproven=- skipped=- | refusing=read,strict_read drive=returned:0 swept=ExtraReviewReaders.Nested.loose:returned,ExtraReviewReaders.read:returned,KEYED.keys()[0]:returned,QUEUED<dict>[deep][0][0]:returned,strict_read.fallback:returned,strict_read:raised:ValueError forced=0'
# ROUND 6'S SECOND ESCAPE, AND A REGRESSION: MASTER'S GUARD REFUSED IT. `extra_review_reader(text,
# options={"object_pairs_hook": one_reading})` refuses when the sweep calls it, because the sweep
# passes nothing for a parameter with a default -- and called with `options={}` it returns PASS with
# an empty findings array. A DEFAULT IS WHAT A CALLER GETS BY PASSING NOTHING, so a refusal a
# default holds is one any caller can take away. Nothing here reads how a default is written: at
# every scan the probe looks at what the scanner refuses WITH -- its decoder, its hook, the class
# MODULE wrote for it, what the hook calls -- and at the defaults of every function of MODULE's on
# the stack; one holding any of those is copied without it, and the run is repeated with that copy
# in place. What the decode then does is the answer. Nine readers hold their refusal in a default --
# a dictionary of keywords, the hook, the decoder class, a decoder, the hook behind a `lambda`, a
# factory's hook, a function made around the hook, which is made again around the copy, a decoder
# the module body builds through a default, which cannot be repeated and is answered where it is
# seen, and a `MappingProxyType` that cannot be copied, which is unproven -- and each is red.
# `by_own_hook` has defaults holding the very same hook and a hooked decoder, and decodes with its
# own: repeated without them it still refuses, and it is green, and so is `by_module_decoder`.
probe_stand_in taken 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


import types


class StrictDecoder(json.JSONDecoder):
    def __init__(self, **kwargs):
        super().__init__(object_pairs_hook=one_reading, **kwargs)


STRICT = json.JSONDecoder(object_pairs_hook=one_reading)


def extra_review_reader(text, options={"object_pairs_hook": one_reading}):
    return json.loads(text, **options)


def by_hook_default(text, hook=one_reading):
    return json.loads(text, object_pairs_hook=hook)


def by_class_default(text, cls=StrictDecoder):
    return json.loads(text, cls=cls)


def by_decoder_default(text, *, decoder=STRICT):
    return decoder.decode(text)


def by_wrapped_default(text, hook=one_reading):
    return json.loads(text, object_pairs_hook=lambda pairs: hook(pairs))


def by_factory_default(hook=one_reading):
    return lambda text: json.loads(text, object_pairs_hook=hook)


def by_frozen_default(text, options=types.MappingProxyType({"object_pairs_hook": one_reading})):
    return json.loads(text, **options)


def strict_for(hook):
    return lambda text: json.loads(text, object_pairs_hook=hook)


def by_closure_default(text, decode=strict_for(one_reading)):
    return decode(text)


def strict_decoder(hook=one_reading):
    return json.JSONDecoder(object_pairs_hook=hook)


BODY_MADE = strict_decoder()


def by_own_hook(text, decoder=STRICT, fallback=one_reading):
    return json.loads(text, object_pairs_hook=one_reading)


def by_module_decoder(text):
    return STRICT.decode(text)
PYSHAPE
probe_expect taken \
  'decoded=yes unrefusing=by_class_default,by_decoder_default,by_factory_default.<locals>.<lambda>,by_hook_default,by_wrapped_default,extra_review_reader,strict_decoder,strict_for.<locals>.<lambda> unproven=by_frozen_default skipped=- | refusing=<module>,StrictDecoder.__init__,by_class_default,by_decoder_default,by_factory_default.<locals>.<lambda>,by_frozen_default,by_hook_default,by_module_decoder,by_own_hook,by_wrapped_default,extra_review_reader,read,strict_decoder,strict_for.<locals>.<lambda> drive=returned:0 swept=StrictDecoder.__init__:raised:TypeError/raised:TypeError/raised:TypeError,by_class_default:raised:ValueError,by_closure_default:raised:ValueError,by_decoder_default:raised:ValueError,by_factory_default:raised:ValueError,by_frozen_default:raised:ValueError,by_hook_default:raised:ValueError,by_module_decoder:raised:ValueError,by_own_hook:raised:ValueError,by_wrapped_default:raised:ValueError,extra_review_reader:raised:ValueError forced=10'
# AND A DEFAULT THAT SELECTS A DECODE WITHOUT HOLDING ONE. `HOOKS[mode]` with `mode="strict"` holds
# nothing a copy could take out, and a caller passing `mode="loose"` decodes unhooked. So a default
# is an input, as the environment is: set, in a repeat of the run that entered its function, to each
# value of its own kind the file holds as a constant and to the kind's own empty value -- which is
# how `index=1` becomes `0`, and a keyword-only `strict=True` becomes `False`. Three red;
# `by_any_mode` reads a table whose every entry, and whose fallback, is the hook, and is green
# whatever it is given.
probe_stand_in selected 'json.loads(block.content, object_pairs_hook=one_reading)' <<'PYSHAPE'


HOOKS = {"strict": one_reading, "loose": None}


def by_selected_hook(text, mode="strict"):
    return json.loads(text, object_pairs_hook=HOOKS[mode])


def by_selected_index(text, index=1):
    return json.loads(text, object_pairs_hook=(None, one_reading)[index])


def by_selected_flag(text, *, strict=True):
    return json.loads(text, object_pairs_hook=(None, one_reading)[strict])


STRICT_HOOKS = {"strict": one_reading, "also": one_reading}


def by_any_mode(text, mode="strict"):
    return json.loads(text, object_pairs_hook=STRICT_HOOKS.get(mode, one_reading))
PYSHAPE
probe_expect selected \
  'decoded=yes unrefusing=by_selected_flag,by_selected_hook,by_selected_index unproven=- skipped=- | refusing=by_any_mode,by_selected_flag,by_selected_hook,by_selected_index,read drive=returned:0 swept=by_any_mode:raised:ValueError,by_selected_flag:raised:ValueError,by_selected_hook:raised:ValueError,by_selected_index:raised:ValueError forced=15'
# AND THE QUESTION ENDS. It runs MODULE's hook, and round 6 held it outside the ten-second bound on
# a run, so a hook that rebuilds `dict(pairs)` until it is as long as `pairs` -- which a repeated
# name never makes true -- held the probe until something outside it killed it, with no report. The
# question is bounded on its own now, and one that has not answered in time is UNPROVEN: red, and
# reported, in the time the bound allows.
probe_stand_in settling 'json.loads(block.content, object_pairs_hook=settled)' <<'PYSHAPE'


def settled(pairs):
    table = dict(pairs)
    while len(table) != len(pairs):
        table = dict(pairs)
    return table
PYSHAPE
probe_expect settling \
  'decoded=yes unrefusing=- unproven=one_reading,read skipped=- | refusing=- drive=returned:0 swept=one_reading:raised:ValueError/raised:ValueError/raised:ValueError forced=1'
# WHAT THIS DOES NOT REACH. Each shape below was written beside a hooked decode and run through this
# probe, which reported nothing unrefusing, nothing unproven and nothing skipped -- and each one
# returns without a refusal on a document naming `findings` twice (for the hook that counts names,
# one with only those two), called directly or, for the scanner dug out of the probe's watch, under
# the probe:
#   * a decode that does not go through the standard library's `json`: `ast.literal_eval`, or a
#     parser written by hand. Nothing is scanned, and `decoded=no` is the whole of what stands
#     between that and a silent pass when it is the only decode, which is what `elsewhere` below is
#     for;
#   * a decode in another process: `subprocess.run([sys.executable, "-c", ...])`;
#   * a branch in code that is not this file's code: a function of a module the parser imports
#     that decodes unhooked when its text starts with `[`, and the same function compiled at run
#     time with `exec`. A scan either one makes while the parser runs is answered for -- `borrowed`
#     above -- but only the code objects compiled from MODULE's own file are held to coverage, so a
#     branch in theirs is not turned;
#   * a value the probe does not vary, in code that ran whole: `getattr(json, name)` with
#     `name="dumps"`, a default set to each of the file's strings and never to `"loads"`, which the
#     file does not hold; and a loop over `re.findall('"verdict"', text)` that sets the hook inside
#     it, and so runs no times for a document with no `verdict` in it. A branch is turned, a
#     variable read from the environment is set to each of the file's own strings, and a default to
#     each value of its own kind the file holds and to its kind's empty value; a value that arrives
#     any other way -- a document's content, a file, the clock -- is taken as it came;
#   * a refusal a default holds only through code: `def by_function_default(text,
#     decode=strict_read)`, whose default is a function that refuses with the module's own hook. A
#     default is searched for what the scanner refused with through what it holds -- its items, its
#     attributes, a function's own defaults and closure -- and never through a function's code or
#     its globals, which reach everything the module has; a caller passing `decode=json.loads` gets
#     the erased value;
#   * a default of a function only the module body runs: `STRICT = make_decoder()` with `def
#     make_decoder(mode="strict")`, selecting from `{"strict": one_reading, "loose": None}`. A run
#     of the module body cannot be repeated with a default changed, because the body makes its
#     functions again, so that default is not varied, and `make_decoder(mode="loose")` decodes
#     unhooked;
#   * a class whose `__module__` names another module: `types.new_class("Readers", ...)` is written
#     in this file and says it is `types`'s. A class is walked only where it says it is MODULE's, so
#     the `staticmethod(json.loads)` inside that one is never called;
#   * the scanner the standard library made, dug out of the closure of the probe's own watch: a
#     module that goes looking for it scans with nothing watching, and under this probe it returns
#     the erased value;
#   * and a hook that raises on the duplicate for something only both values together make. One
#     refusing any object of more than two names raises on it, reads both readings, and is counted
#     as refusing.
# AND WHAT IT REFUSES AND SHOULD NOT, OR MIGHT NOT -- because a guard is only honest if both sides
# of it are written down. Each was run through this probe and reported red; the first, second and
# fourth do what a parser should, the third is the cost of reading nothing into a value a function
# returns, and the fifth the cost of asking a default only through a copy of it:
#   * a decoder whose hook is `list`, keeping the pairs the scanner built, with its own `decode`
#     walking them and raising on a repeated name. The scanner returns, so the pair reports
#     UNREFUSING -- accurately, of the scanner -- while the reader refuses. Rounds 4 and 5 red it
#     too. The only way to ask it is through the method it declares, which is round 3's defect;
#   * a property nothing reads whose getter calls anything at all -- `return self.text.upper()` --
#     is code in the file that never ran, and unproven. The remedy is a drive that reads it;
#   * a function that catches a refusal and returns anything but the refusal -- an error value,
#     `None` -- is counted with the unrefusing, as round 5 counted it: nothing here reads what the
#     returned value means;
#   * a helper that reads an attribute of its argument before it does anything else -- `def
#     path_label(path): return path.as_posix()` -- raises on every document the probe hands it, as
#     text or as a path string, never runs past that line, and is unproven. The remedy is a drive
#     that calls it with what it takes;
#   * and a reader whose default holds its hook in something `copy.deepcopy` cannot copy -- a
#     `MappingProxyType` of keywords -- cannot be asked without the hook, and is unproven whether or
#     not any caller would ever pass that parameter.
# And `borrowed` above is a fourth, stated there: a library the parser calls that decodes a
# repeated name for its own reasons is answered for as the parser, and red.
# Round 5's other false red, a decoder whose `scan_once` takes its index by keyword only, is green
# here: the question is put to the scanner the standard library made, and that one takes its index
# positionally.
cat > "$tmp/probe-elsewhere.py" <<'PYSHAPE'
import ast


class Block:
    def __init__(self, content):
        self.content = content


def read(text, block):
    return ast.literal_eval(block.content)


def main(argv):
    read("", Block(open(argv[-1]).read()))
    return 0
PYSHAPE
probe_expect elsewhere \
  'decoded=no unrefusing=- unproven=- skipped=- | refusing=- drive=returned:0 swept=- forced=0'
# AND THEN THE PARSER THE AUDIT ACTUALLY RUNS, DRIVEN OVER ITS OWN WORK. Eight real parses: the
# workflow form in both renderings, the repeated name it must refuse, the same at three objects
# down, the older bare object, the frontier form, the frontier form quoting a fenced example, and
# the ledger subcommand. They are drives rather than assertions -- what the first seven return is
# asserted by the families above, on these same shapes -- and their job here is that the parser's
# own code runs, and every scan it makes is answered for. What they leave, the probe takes: a
# handler the eight never enter is entered, and a branch they take one way only is turned --
# measured when this was written, all 118 conditional branches in the 41 code objects of
# `scripts/pr-review-parse.py` went both ways, over 109 altered runs -- and the verdict line below
# asserts that every instruction in the file ran. A loop no document enters is the one thing
# turning cannot reach -- `verdict_candidates` reads the bare object only inside one, which is why
# the bare drive is here -- so if a change leaves one behind, give it a drive rather than a filter.
#
# THE QUOTED DRIVE IS THAT RULE, APPLIED. The parser reads a fence run inside a block's content only
# inside a loop over that content, and resolves a reference in an info string only inside the
# callback `rendered_language` hands `re.sub`. None of the other seven documents reaches either,
# and with only those seven this line read
# `unproven=referable,rendered_language.<locals>.resolved,unresolved_material`. The example's nested
# fence enters the loop, and without it `unresolved_material` stays unproven. Its info string holds
# one of each path the callback takes: a backslash escape, a named reference, a name no table holds,
# a decimal and a hex reference, and a reference to a control character, which is left as written.
# With one decimal reference alone, `rendered_language.<locals>.resolved` stayed unproven. What it
# returns is asserted nowhere, and nothing turns on it: the parser reads it as a prose review.
printf '<!-- upstroke-frontier-review pr=1 -->\nReviewed head: %s\n\n%s\n\n' \
  "$revived_head" '1. **P2 -- a finding.** Detail.' > "$tmp/probe-drive-prose.md"
printf 'VERDICT: CHANGES_REQUIRED\n' >> "$tmp/probe-drive-prose.md"
printf '%s\n\n| ID | Disposition |\n| --- | --- |\n| PR1-A | fixed |\n' \
  '## Review finding ledger' > "$tmp/probe-drive-ledger.md"
{ printf '<!-- upstroke-frontier-review pr=1 -->\nReviewed head: %s\n\n' "$revived_head"
  printf '````text\n```%s\nan example quoted inside an example\n```\n````\n\n' \
    '\*&amp;&notareal;&#110;&#x6a;&#11;'
  printf '1. **P2 -- a finding.** Detail.\n\nVERDICT: CHANGES_REQUIRED\n'
} > "$tmp/probe-drive-quoted.md"
# Only the verdict line is asserted, and deliberately: which functions refuse and what the drives
# returned are the parser's own business -- a second decoder that genuinely refuses adds its name
# to them and must stay green. `decoded=yes` is the limb that makes the rest mean something: a
# parser with no decode left for the probe to watch satisfies every count and fails here.
got="$(decode_probe scripts/pr-review-parse.py \
  "review|$tmp/one-findings.md" "review --nul|$tmp/one-findings.md" \
  "review|$tmp/dup-findings.md" "review|$tmp/dup-deep.md" "review|$tmp/one-bare.md" \
  "review|$tmp/probe-drive-prose.md" "review|$tmp/probe-drive-quoted.md" \
  "ledger --nul|$tmp/probe-drive-ledger.md")"
[[ "${got%% | *}" == 'decoded=yes unrefusing=- unproven=- skipped=-' ]] \
  || error "MUT-JSON-REPEATED-NAME-CHOSEN: got [$got], want the verdict [decoded=yes unrefusing=- unproven=- skipped=-]"
# THAT THE SWEEP IS EMPTY IS NOT ASSERTED, and the reason is round 2's finding: an empty sweep
# asserted on its own says that no function may be added to this parser unless a drive runs it, and
# `def print_review(value)` -- a helper that encodes and never decodes -- is required-green by round
# 2's finding. It is swept, every instruction of it runs, and it must stay green.


# --- what a block SWALLOWS is material, and so is a severity only a decoder can read ------------
# TWO FINDINGS, ONE BLIND SPOT, WHICH IS WHY THEY ARE ONE SECTION. `unresolved_material` accounted
# for every fence run OUTSIDE the blocks the structure scan found, and `verdict_candidates` looked
# for the bare form's object opener in exactly the same place -- so the CONTENT of a block neither
# of them read the verdict from was inert to both, and a verdict sitting in it was invisible twice.
#
# It is not inert. CommonMark suppresses a fence inside a raw-HTML block and this scan does not
# (https://spec.commonmark.org/0.31.2/#html-blocks), so `<!--`, a ```text line and `-->` open a
# block HERE that a reader of the comment never sees, and that block SWALLOWS the real verdict as
# its content. Every fence run stays accounted for -- each one WAS consumed as a fence, just into
# the wrong block, and accounting for runs says nothing about which block each went into -- the
# swallowed object is not a candidate, and a clean `PASS` appended after it is the only candidate
# left. Measured on this file's own stub with the head replaced by a real commit:
# READY, exit 0, verdict=PASS, zero findings, no blocker at all, out of a comment `markdown-it-py`
# 3.0.0 parses to exactly one fenced verdict -- CHANGES_REQUIRED, carrying a P1.
#
# The same blind spot counted from the other end is the second finding: a `text`-fenced
# `role_understanding` object is not a candidate either, so the count reaches ZERO -- and zero was a
# road to the prose parser, which read the `VERDICT: PASS` line written underneath it. One repair
# closes both, because a block the verdict was not read FROM is now accounted for on the same terms
# as the material outside one: a fence run naming `json` anywhere in its content -- at the start of
# a line or behind a blockquote's `>` or a list marker -- and the bare form's object opener, are
# material and there is no result.
#
# AND THE UNCLOSED `<!--` IS A SPELLING, NOT THE CLASS. `<div>` (HTML block type 6, ended by a blank
# line), `<pre>` (type 1, ended by its closing tag) and any complete tag on a line of its own start
# HTML blocks too. Each is a case below, with the swallowing fence spelled six ways, because a rule
# that closed the one tag the finding named would be the recogniser made cleverer again.
swallow_blocking() {  # swallow_blocking: the blocking object every case below hides
  printf '{"reviewed_sha":"%s","base_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P\\u0031"}]}' \
    "$revived_head" "$revived_base"
}
swallow_pass() {  # swallow_pass: the candidate a swallow leaves standing as the only one
  printf '{"reviewed_sha":"%s","base_sha":"%s","verdict":"PASS","findings":[]}' \
    "$revived_head" "$revived_base"
}
# The reported shape: an unclosed `<!--`, whose hidden ```text fence runs through the real verdict.
{ printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n```json\n' "$revived_head"
  swallow_blocking; printf '\n```\n\n<!--\n\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-html-comment.md"
# `<div>`, whose HTML block ends at the blank line, so the fence above it swallows what follows.
{ printf 'Reviewed head: %s\n\n<div>\n```text\n\n```json\n' "$revived_head"
  swallow_blocking; printf '\n```\n\n<div>\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-div.md"
# `<pre>`, whose HTML block ends at `</pre>`.
{ printf 'Reviewed head: %s\n\n<pre>\n```text\n</pre>\n\n```json\n' "$revived_head"
  swallow_blocking; printf '\n```\n\n<pre>\n\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-pre.md"
# A tilde fence swallows the same way, and the swallowing block may name no language at all.
{ printf 'Reviewed head: %s\n\n<!--\n~~~text\n-->\n\n```json\n' "$revived_head"
  swallow_blocking; printf '\n~~~\n\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-tilde.md"
{ printf 'Reviewed head: %s\n\n<!--\n```\n-->\n\n```json\n' "$revived_head"
  swallow_blocking; printf '\n```\n\n<!--\n\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-plain.md"
# The swallowed fence's info string is read the way every other one here is read: its rendered
# language, folded -- so `JSON` is the same shape and an indent of up to three spaces is a fence.
{ printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n```JSON\n' "$revived_head"
  swallow_blocking; printf '\n```\n\n<!--\n\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-uppercase.md"
{ printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n   ```json\n   ' "$revived_head"
  swallow_blocking; printf '\n   ```\n\n<!--\n\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-indented.md"
# CRLF endings are the same comment: GitHub stores what the reviewer posted, and the fence the
# reader saw closed the block.
sed 's/$/\r/' "$tmp/swallow-html-comment.md" > "$tmp/swallow-crlf.md"
# The older bare form has no fence to hide, so what is swallowed is its object opener.
{ printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n' "$revived_head"
  printf '{"role_understanding":"x","verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P\\u0031"}]}'
  printf '\n```\n\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-bare-object.md"
# THE SECOND FINDING, whose candidate count reached zero and whose comment went to the prose parser.
{ printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n```text\n' "$revived_head"
  printf '{"role_understanding":"x","reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P\\u0031","failing_test":"a_test_that_fails"}]}' \
    "$revived_head"
  printf '\n```\n\nVERDICT: PASS\n'
} > "$tmp/swallow-prose-object.md"
# and the swallow reached through the prose form as well, which is the same comment claiming both.
{ printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n<!--\n```text\n-->\n\n```json\n' \
    "$revived_head"
  swallow_blocking; printf '\n```\n\nVERDICT: PASS\n'
} > "$tmp/swallow-prose-swallowed.md"
# AND THE `json` FENCE A SWALLOW HIDES NEED NOT START ITS LINE. Inside a blockquote or a list item
# it is still the verdict to CommonMark, and inside the hidden block it was nothing to a content
# rule anchored at the start of the line: the review of that rule's head put `> ` in front of the
# blocking object's fence and the audit read PASS out of the same swallow. The object lists no
# findings, because CHANGES_REQUIRED listing none blocks on its own -- so there is no stray severity
# to rescue the case, and what is asserted is the swallow alone. `markdown-it-py` 3.0.0 reads each
# of these three comments as exactly one fenced `json` block, and it is the CHANGES_REQUIRED one.
swallow_quiet() {  # swallow_quiet: a blocking object with nothing in it for the stray scan to find
  printf '{"reviewed_sha":"%s","base_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[]}' \
    "$revived_head" "$revived_base"
}
{ printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n> ```json\n> ' "$revived_head"
  swallow_quiet; printf '\n> ```\n\n<!--\n```\n-->\n\n<!--\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-blockquote.md"
# A list item's closing fence would close a three-backtick block here, so the hidden one is longer.
{ printf 'Reviewed head: %s\n\n<!--\n````text\n-->\n\n- ```json\n  ' "$revived_head"
  swallow_quiet; printf '\n  ```\n\n<!--\n````\n-->\n\n<!--\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-list-item.md"
{ printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n> - ```json\n>   ' "$revived_head"
  swallow_quiet; printf '\n>   ```\n\n<!--\n```\n-->\n\n<!--\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/swallow-quoted-list.md"
for shape in html-comment div pre tilde plain uppercase indented crlf bare-object prose-object \
             prose-swallowed blockquote list-item quoted-list; do
  got="$(review_rows "$tmp/swallow-$shape.md")"
  [[ "$got" == 0\|* ]] \
    && error "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape]: a comment whose verdict was swallowed parsed, got [$got]"
  [[ "$got" == *PASS* ]] \
    && error "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape]: the hidden PASS was taken as the verdict, got [$got]"
  # and nothing was written where a caller ignoring the status would read it
  expect "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape] payload" \
    "$(parse_nul review "$tmp/swallow-$shape.md")" '1|'
  # AND IT REFUSED FOR THE REASON THIS CASE IS ABOUT: a parse that fails here because some other
  # rule of the parser moved is not a witness to anything.
  contains "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape]" "$(parse_why "$tmp/swallow-$shape.md")" \
    "could not account for as a block"
  # Through main, because the parser refusing is only half of it: READY, with no blocker at all,
  # is what the audit did with every one of these.
  got="$(STUB_REVIEW_BODY="$tmp/swallow-$shape.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
  contains "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape]" "$got" "review-parse-failed"
  contains "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape]" "$got" "NOT-READY"
  [[ "$got" == *"verdict=PASS"* ]] \
    && error "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape]: the audit read PASS out of it"
done
# The diagnostic names WHICH of the two shapes it found, so each finding's own witness is pinned to
# its own rule rather than to whichever of them fires first -- and the three whose fence does not
# start its line are pinned to the content rule, not to a fence run left outside every block.
for shape in html-comment blockquote list-item quoted-list; do
  contains "MUT-BLOCK-CONTENT-NOT-MATERIAL [$shape]" "$(parse_why "$tmp/swallow-$shape.md")" \
    '```json inside the block'
done
contains MUT-BLOCK-CONTENT-NOT-MATERIAL "$(parse_why "$tmp/swallow-prose-object.md")" \
  'a verdict object inside the block'
# THE CONTROLS, so no case above can pass on a fixture this parser refuses anyway. The same
# blocking object as an ordinary fenced block is read, recorded and blocking; the same prose
# comment with its finding written the prose form's own way is read, recorded and blocking.
{ printf 'Reviewed head: %s\n\n```json\n' "$revived_head"; swallow_blocking; printf '\n```\n'; } \
  > "$tmp/swallow-control-json.md"
expect MUT-BLOCK-CONTENT-NOT-MATERIAL "$(review_rows "$tmp/swallow-control-json.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/$revived_base/-;P1:CRITICAL:0"
got="$(STUB_REVIEW_BODY="$tmp/swallow-control-json.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-BLOCK-CONTENT-NOT-MATERIAL "$got" "open-P1:CRITICAL"
[[ "$got" == *review-parse-failed* ]] \
  && error "MUT-BLOCK-CONTENT-NOT-MATERIAL: the control review was refused, got [$got]"
printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n1. **P1 - a thing that blocks.** Detail.\n\nVERDICT: PASS\n' \
  "$revived_head" > "$tmp/swallow-control-prose.md"
expect MUT-BLOCK-CONTENT-NOT-MATERIAL "$(review_rows "$tmp/swallow-control-prose.md")" \
  "0|prose/$revived_head/PASS/-/-;P1:-:0"
# AND THE RULE IS NOT "A FENCE INSIDE A BLOCK", which would refuse a review for quoting one. A
# longer fence around a shorter one is how a `text` block shows a fenced block, and only a fence
# run naming `json` is material -- `json` being the one shape a verdict is ever read from. This
# comment carries a nested fence and parses as the prose review it is, so the cases above are about
# what the swallowed block HELD and not about nesting.
{ printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n' "$revived_head"
  printf '````text\nan example of a block:\n```text\nnot a verdict\n```\n````\n\nVERDICT: PASS\n'
} > "$tmp/nested-text-fence.md"
expect MUT-BLOCK-CONTENT-NOT-MATERIAL "$(review_rows "$tmp/nested-text-fence.md")" \
  "0|prose/$revived_head/PASS/-/-"
# NOR IS IT "A BLOCKQUOTE INSIDE A BLOCK". Quoted behind `> `, the same nested fence names `text`
# and the comment parses: what the blockquote and list cases above turn on is the `json`, not `>`.
{ printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n' "$revived_head"
  printf '````text\n> ```text\n> a quoted block, not a verdict\n> ```\n````\n\nVERDICT: PASS\n'
} > "$tmp/quoted-text-fence.md"
expect MUT-BLOCK-CONTENT-NOT-MATERIAL "$(review_rows "$tmp/quoted-text-fence.md")" \
  "0|prose/$revived_head/PASS/-/-"

# --- a guard reads what its consumer reads, not what the comment spells -------------------------
# MUT-GUARD-READS-THE-WRITTEN-SPELLING. The section above made a block's content material, and both
# of its rules then asked their question of the RAW CHARACTERS while the only consumer of what they
# guard asks it of what those characters DECODE TO. A check that reads bytes its consumer will
# later decode is comparing two different documents, and the gap between them is exactly where
# something material can stand.
#
# TWO DECODERS, ONE PER RULE, and each is the whole of what its own consumer does.
#
#   * an INFO STRING is read by a renderer, which resolves backslash escapes and character
#     references in it and nothing else -- the info string is not parsed as Markdown, so `js*on*`
#     is `js*on*` and `js<span></span>on` is itself (measured against `markdown-it-py` 3.0.0) --
#     and gives the block the first word of what is left as its language. So
#     ```` ```jso&#110; ```` opens the one
#     `language-json` block the comment renders to, carrying the blocking verdict inside a
#     swallowing block, while the rule that exists to find exactly that saw no `json` fence at all.
#     Measured on this file's own stub: exit 0, verdict=PASS, READY, one merge call;
#   * a BARE OBJECT'S KEY is read by `json.loads`, which resolves the string's escapes. So
#     `{"role_understandin\u0067":...}` opens an object whose first key is `role_understanding` to
#     that reader, to GitHub's renderer and to the person reading the comment, and was no opener to
#     a pattern matching the name's characters -- the candidate count reached ZERO, which is the
#     road to the prose parser and the `VERDICT: PASS` written underneath. Measured the same way:
#     exit 0, prose PASS, READY, one merge call.
#
# EACH CASE HAS ITS LITERAL TWIN, so what is asserted is the spelling and not the shape, and the
# controls below say what the rule is NOT: a reference is not itself material, one that resolves to
# something other than `json` leaves the comment readable, and one CommonMark does not recognise --
# no semicolon, no such name -- is left as written and is not a name.
enc_swallow() {  # enc_swallow INFO: the reported swallow, with the hidden fence's language INFO
  printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n```%s\n' "$revived_head" "$1"
  swallow_quiet; printf '\n```\n\n<!--\n\n```json\n'; swallow_pass; printf '\n```\n'
}
# Every spelling of the reference grammar, and the one place it decides where a word ENDS.
enc_swallow 'jso&#110;'                  > "$tmp/enc-decimal.md"
enc_swallow '&#x6a;son'                  > "$tmp/enc-hex.md"
enc_swallow '&#X6A;SO&#78;'              > "$tmp/enc-upper.md"
enc_swallow '&#106;&#115;&#111;&#110;'   > "$tmp/enc-whole.md"
enc_swallow '&Tab;json'                  > "$tmp/enc-named.md"
enc_swallow 'json&#32;extra'             > "$tmp/enc-first-word.md"
# and the spelling together with the line position the round before this one closed: a `json` fence
# behind a blockquote's `>`, inside the swallow, with its language written as a reference.
{ printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n> ```jso&#110;\n> ' "$revived_head"
  swallow_quiet; printf '\n> ```\n\n<!--\n```\n-->\n\n<!--\n```json\n'; swallow_pass; printf '\n```\n'
} > "$tmp/enc-blockquote.md"
# THE BARE FORM'S KEY, escaped at its end, at its start and throughout, inside the swallowing block
# and loose in a marker-bearing comment -- the two places the opener is looked for.
enc_key_tail='{"role_understandin\u0067":"x","verdict":"CHANGES_REQUIRED","findings":[]}'
enc_key_head='{"\u0072ole_understanding":"x","verdict":"CHANGES_REQUIRED","findings":[]}'
enc_key_spaced='{ "\u0072\u006fle_understandin\u0067":"x","verdict":"CHANGES_REQUIRED","findings":[]}'
for shape in tail head spaced; do
  name="enc_key_$shape"; object="${!name}"
  { printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n```text\n%s\n```\n\nVERDICT: PASS\n' \
      "$revived_head" "$object"; } > "$tmp/enc-key-block-$shape.md"
  { printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n%s\n\nVERDICT: PASS\n' \
      "$revived_head" "$object"; } > "$tmp/enc-key-loose-$shape.md"
done
for shape in decimal hex upper whole named first-word blockquote \
             key-block-tail key-block-head key-block-spaced \
             key-loose-tail key-loose-head key-loose-spaced; do
  got="$(review_rows "$tmp/enc-$shape.md")"
  [[ "$got" == 0\|* ]] \
    && error "MUT-GUARD-READS-THE-WRITTEN-SPELLING [$shape]: a comment whose verdict was hidden by a spelling parsed, got [$got]"
  [[ "$got" == *PASS* ]] \
    && error "MUT-GUARD-READS-THE-WRITTEN-SPELLING [$shape]: the hidden PASS was taken as the verdict, got [$got]"
  expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING [$shape] payload" \
    "$(parse_nul review "$tmp/enc-$shape.md")" '1|'
  # Through main, because the parser refusing is only half of it: READY and a merge call is what
  # the audit did with each of these.
  got="$(STUB_REVIEW_BODY="$tmp/enc-$shape.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [$shape]" "$got" "NOT-READY"
  [[ "$got" == *"verdict=PASS"* ]] \
    && error "MUT-GUARD-READS-THE-WRITTEN-SPELLING [$shape]: the audit read PASS out of it"
done
# AND EACH REFUSED FOR THE REASON ITS OWN CASE IS ABOUT. The swallowed ones are the content rule
# reading a resolved info string; the loose ones are the candidate count, which the escaped key had
# emptied, put back to one against a comment that also carries the prose form's marker.
for shape in decimal hex upper whole named first-word blockquote; do
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [$shape]" "$(parse_why "$tmp/enc-$shape.md")" \
    "inside the block"
done
for shape in tail head spaced; do
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [key-block-$shape]" \
    "$(parse_why "$tmp/enc-key-block-$shape.md")" 'a verdict object inside the block'
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [key-loose-$shape]" \
    "$(parse_why "$tmp/enc-key-loose-$shape.md")" "the prose form's marker and 1 verdict block"
done
# THE CONTROLS. A reference is not itself material: one that resolves to something other than
# `json` -- a different language, or a word that merely starts the same -- leaves the comment
# readable, and so does one CommonMark does not recognise, which is left exactly as written.
enc_swallow 'te&#120;t'        > "$tmp/enc-control-text.md"
enc_swallow 'jso&NewLine;'     > "$tmp/enc-control-jso.md"
enc_swallow 'jso&#110'         > "$tmp/enc-control-nosemi.md"
enc_swallow 'jso&notareal;n'   > "$tmp/enc-control-unknown.md"
enc_swallow 'js&#38;on'        > "$tmp/enc-control-amp.md"
for shape in text jso nosemi unknown amp; do
  expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING control [$shape]" \
    "$(review_rows "$tmp/enc-control-$shape.md")" \
    "0|json/$revived_head/PASS/$revived_base/-"
done
# A KEY THAT IS NOT THE NAME AFTER DECODING IS NOT THE OPENER, and an escape this cannot resolve is
# left as written rather than dropped -- so neither of these empties anything or refuses anything.
for object in '{"role_understandin\u0068":"x"}' '{"role_understandin\q":"x"}' '{"role_understanding_x":"x"}'; do
  printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n%s\n\nVERDICT: PASS\n' \
    "$revived_head" "$object" > "$tmp/enc-key-control.md"
  expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING key control [$object]" \
    "$(review_rows "$tmp/enc-key-control.md")" "0|prose/$revived_head/PASS/-/-"
done
# AND THE RULE IS NOT "REFUSE WHAT IS SPELLED WITH A REFERENCE OR AN ESCAPE". Each of these is the
# comment's ONE place a verdict can be read from, written the encoded way, and each is READ: the
# `jso&#110;` block is the block a reader sees, and the bare object's first key is the name. A
# guard that refused them would have stopped reading the comment rather than started.
{ printf 'Reviewed head: %s\n\n```jso&#110;\n' "$revived_head"; swallow_blocking; printf '\n```\n'; } \
  > "$tmp/enc-sole-fence.md"
expect MUT-GUARD-READS-THE-WRITTEN-SPELLING "$(review_rows "$tmp/enc-sole-fence.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/$revived_base/-;P1:CRITICAL:0"
printf 'Reviewed head: %s\n\n{"role_understandin\\u0067":"x","reviewed_sha":"%s","base_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P2"}]}\n' \
  "$revived_head" "$revived_head" "$revived_base" > "$tmp/enc-sole-object.md"
expect MUT-GUARD-READS-THE-WRITTEN-SPELLING "$(review_rows "$tmp/enc-sole-object.md")" \
  "0|json/$revived_head/CHANGES_REQUIRED/$revived_base/-;P2:CRITICAL:0"
# and two places are still two, however each is spelled: a resolved `json` fence beside a written
# one is the count refusing, not a recogniser choosing.
{ printf 'Reviewed head: %s\n\n```jso&#110;\n' "$revived_head"; swallow_blocking
  printf '\n```\n\n```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/enc-two-candidates.md"
expect MUT-GUARD-READS-THE-WRITTEN-SPELLING "$(review_rows "$tmp/enc-two-candidates.md")" '1|'
contains MUT-GUARD-READS-THE-WRITTEN-SPELLING "$(parse_why "$tmp/enc-two-candidates.md")" \
  "2 places a verdict could be read from"

# --- the severity the stray scan could not read -------------------------------------------------
# MUT-STRAY-TOKEN-ENCODED. `stray_summary` exists to catch a blocking severity written where the
# findings are not, and it ran over RAW TEXT while the severity only exists after JSON decoding:
# `"severity":"P\u0031"` is `P1` to `json.loads`, to GitHub's renderer and to the person reading
# the comment, and carries no `P1` at all for a regex over the characters. Every witness in this
# family spells it that way, which is exactly why -- the truncation revival, the indented fence,
# the repeated name, and both of the findings this section is about. The scan now reads both
# spellings, what the comment writes and what a decoder reads.
#
# The case that needs it is the residue of the section above: a `text`-fenced object whose first
# key is NOT `role_understanding` and whose content holds no `json` fence line is material to
# neither new rule, so the stray scan is the whole of what is left standing between it and READY.
{ printf 'Reviewed head: %s\n\n```text\n' "$revived_head"; swallow_blocking
  printf '\n```\n\n```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/stray-escaped.md"
expect MUT-STRAY-TOKEN-ENCODED "$(review_rows "$tmp/stray-escaped.md")" \
  "0|json/$revived_head/PASS/$revived_base/P1"
got="$(STUB_REVIEW_BODY="$tmp/stray-escaped.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-STRAY-TOKEN-ENCODED "$got" "manual:P1-outside-the-verdict-object"
[[ "$got" == *READY\ * && "$got" != *NOT-READY* && "$got" != *MANUAL* ]] \
  && error "MUT-STRAY-TOKEN-ENCODED: an escaped severity outside the verdict object reached READY: [$got]"
# The control writes the same severity literally and must give the SAME field, so the case above is
# about the spelling and not about where the object sits.
{ printf 'Reviewed head: %s\n\n```text\n' "$revived_head"
  printf '{"reviewed_sha":"%s","base_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P1"}]}' \
    "$revived_head" "$revived_base"
  printf '\n```\n\n```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/stray-literal.md"
expect MUT-STRAY-TOKEN-ENCODED "$(review_rows "$tmp/stray-literal.md")" \
  "0|json/$revived_head/PASS/$revived_base/P1"
# MUST is read the same way, and so is a severity written with both characters escaped.
{ printf 'Reviewed head: %s\n\n' "$revived_head"
  printf 'The object said "\\u004dUST" and "\\u00503" and nothing else.\n\n'
  printf '```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/stray-encoded-must.md"
expect MUT-STRAY-TOKEN-ENCODED "$(review_rows "$tmp/stray-encoded-must.md")" \
  "0|json/$revived_head/PASS/$revived_base/MUST/P3"
# AND A DOUBLED BACKSLASH IS NOT AN ESCAPE, which is the other direction this can be wrong in: a
# scan that resolved `\uXXXX` wherever it appeared would read the `\u0050` inside `\\u00501` --
# which is a backslash, and then the literal text `u00501` -- as a `P`, and hand the `1` after it a
# word boundary it does not have: a clean review sent to a person for a `P1` nobody wrote.
#
# THE SPELLING HAS TO BE ONE THAT BECOMES A TOKEN WHEN IT IS MISREAD, or this row is green in
# both readings and asserts nothing. It was `\\u0031`, which a loose scan reads as a backslash and
# a `1` -- and a `1` is not a token, so the misdecoding this case exists to catch passed it. The
# loose reading of the text below is `\P1`, and `\b` holds against a backslash.
{ printf 'Reviewed head: %s\n\n' "$revived_head"
  printf '%s\n\n' 'The literal text `\\u00501` is a backslash and then u00501.'
  printf '```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/stray-doubled.md"
expect MUT-STRAY-TOKEN-ENCODED "$(review_rows "$tmp/stray-doubled.md")" \
  "0|json/$revived_head/PASS/$revived_base/-"
# AND JSON'S TWO-CHARACTER ESCAPES ARE RESOLVED, WHICH IS WHAT MAKES A TOKEN A TOKEN. `\t` is a tab
# to `json.loads`, to GitHub's renderer and to the person reading the comment; unresolved it is the
# two letters `\t`, and `tP1` gives `\bP1\b` no boundary to match at. A decoder that resolved only
# `\uXXXX` therefore found NOTHING in a `{"severity":"\tP1"}` written outside the verdict object, and
# every case above still passed: each of them spells its severity `P\u0031`, which that decoder
# still resolves. Measured: MANUAL became READY, with a merge call.
{ printf 'Reviewed head: %s\n\n' "$revived_head"
  printf '%s\n\n' 'A loose object: {"severity":"\tP1"} and nothing else.'
  printf '```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/stray-two-char.md"
expect MUT-STRAY-TOKEN-ENCODED "$(review_rows "$tmp/stray-two-char.md")" \
  "0|json/$revived_head/PASS/$revived_base/P1"
got="$(STUB_REVIEW_BODY="$tmp/stray-two-char.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-STRAY-TOKEN-ENCODED "$got" "manual:P1-outside-the-verdict-object"
# The control writes the tab itself -- the same text after decoding -- and gives the same field, so
# the case above is about the spelling and not about where the object sits.
{ printf 'Reviewed head: %s\n\n' "$revived_head"
  printf 'A loose object: {"severity":"\tP1"} and nothing else.\n\n'
  printf '```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/stray-two-char-literal.md"
expect MUT-STRAY-TOKEN-ENCODED "$(review_rows "$tmp/stray-two-char-literal.md")" \
  "0|json/$revived_head/PASS/$revived_base/P1"
# An escape this cannot resolve is left as it was written rather than dropped, so nothing hides in
# the gap: neither of these carries a token, and neither raises.
{ printf 'Reviewed head: %s\n\n' "$revived_head"
  printf 'Not escapes: "\\uZZZZ", "\\q", "\\ud835\\udfcf", a trailing backslash \\\n\n'
  printf '```json\n'; swallow_pass; printf '\n```\n'; } > "$tmp/stray-unresolvable.md"
expect MUT-STRAY-TOKEN-ENCODED "$(review_rows "$tmp/stray-unresolvable.md")" \
  "0|json/$revived_head/PASS/$revived_base/-"
expect MUT-STRAY-TOKEN-ENCODED "$(parse_nul review "$tmp/stray-unresolvable.md")" \
  "0|review|json|$revived_head|PASS|$revived_base|-|0|"

# --- a name no rendering of the comment contains, and the merge call two witnesses bought --------
# TWO THINGS ARE ASSERTED HERE AND THE HARNESS IS WHY THEY SHARE A SECTION. Everything above reads
# the parser's exit and the audit's state; a bypass does not stop at a state, it ends in
# `gh pr merge`, and none of the fixtures above can count that call because none of them reaches
# READY: the audit resolves the reviewed and base commits with git, and their shas are invented.
# So the harness below is a REPOSITORY OF ITS OWN -- two empty commits, `origin/master` at the
# second, the stub's pull request head the same commit -- and its clean control reaches READY and
# MAKES the call. That is what makes a zero mean something: a count that is zero for every input,
# including the ones that must not be zero, asserts nothing at all.
#
# MUT-GUARD-READS-THE-WRITTEN-SPELLING, at the cost. The first two comments are the review's own
# witnesses against the head that first claimed this finding closed, kept as they were written: a
# blocking verdict behind a ```` ```jso&#110; ```` fence inside a swallowing block, and a
# `text`-fenced object opening `{"role_understandin\u0067"`. At that head each one
# parsed, reported PASS, reached READY and made one merge call, out of a review that blocks. They
# are fixtures rather than something a reviewer re-checks because the review that answers "is this
# actually fixed" refused eight consecutive times on the head this repairs; a fixture answers the
# same question on every head afterwards and cannot refuse.
#
# MUT-NAME-FOUND-IN-NO-RENDERING is the same guard over-corrected, and the direction that costs a
# reviewer the review instead of letting one past. `json&#133;x` was resolved to U+0085 and split
# there, so the first word read as `json` and an ordinary example block became a second candidate:
# READY to NOT-READY and the merge call to none, for a comment that says nothing wrong and that no
# rewriting clears. Measured, `markdown-it-py` 3.0.0 renders that fence `language-json&#133;x`; so
# does `json&amp;#133;x`, byte for byte, and the two are read the same way here. `json&#11;x` is the
# same defect at U+000B; a case here once required it to refuse, and it is asserted green below.
enqueue_repo="$tmp/enqueue-repo"
git init -q "$enqueue_repo"
git -C "$enqueue_repo" -c user.email=t@example -c user.name=t \
  commit -q --allow-empty -m "the base the review was made against"
enqueue_base="$(git -C "$enqueue_repo" rev-parse HEAD)"
git -C "$enqueue_repo" -c user.email=t@example -c user.name=t \
  commit -q --allow-empty -m "the head the review read"
enqueue_head="$(git -C "$enqueue_repo" rev-parse HEAD)"
git -C "$enqueue_repo" remote add origin "$enqueue_repo"
git -C "$enqueue_repo" update-ref refs/remotes/origin/master "$enqueue_head"
# The stub the audit is run against: the `lookup` one above, plus the two reads that stand between
# READY and the enqueue, plus `pr merge` ITSELF, which is recorded rather than answered. Nothing
# here reaches the network -- `origin` is the harness repository itself -- and an unmodelled call
# is still `GH-UNSTUBBED`, so a case cannot pass by taking a path this does not describe.
enqueue_gh="$tmp/enqueue-gh"
mkdir -p "$enqueue_gh"
printf '#!/usr/bin/env bash\nhead=%s\n' "$enqueue_head" > "$enqueue_gh/gh"
cat >> "$enqueue_gh/gh" <<'GH'
case "$*" in
  "repo view"*)               echo eventloops/upstroke ;;
  *"--jq .owner.login")       echo eventloops ;;
  *"--jq .owner.type")        echo User ;;
  *"/labels?per_page=100"*)   ;;
  "label create"*)            ;;
  "pr edit"*)                 ;;
  "pr merge"*)                printf '[%s]' "$@" >> "$STUB_MERGE_LOG"
                              printf '\n' >> "$STUB_MERGE_LOG" ;;
  *rulesets*)                 ;;
  *check-runs*)               printf 'upstroke-ci\t10\tsuccess\nupstroke-pr-policy\t11\tsuccess\n' ;;
  *"/pulls?state=open"*)      exit 0 ;;
  *timeline*)                 ;;
  *"/comments?per_page=100"*) echo "2026-09-01T00:00:00Z 5001" ;;
  *"/issues/comments/5001 --jq .created_at") echo 2026-09-01T00:00:00Z ;;
  *"/issues/comments/5001 --jq .body")       cat "$STUB_REVIEW_BODY" ;;
  *"--json body"*)            echo "no ledger" ;;
  *"--json headRefOid"*)      echo "$head" ;;
  *"--json baseRefName"*)     echo master ;;
  "pr view"*)                 printf '%s\n%s\nfalse\nCLEAN\nmaster\n%s\n0\n' \
                                feature/x "$head" "$head" ;;
  *) echo "GH-UNSTUBBED $*" >&2; exit 97 ;;
esac
GH
chmod +x "$enqueue_gh/gh"
merge_run() {  # merge_run FILE: "<status>|<output, on one line>|<gh pr merge calls>"
  local out status=0
  : > "$tmp/merge-calls.log"
  out="$(cd "$enqueue_repo" && STUB_MERGE_LOG="$tmp/merge-calls.log" STUB_REVIEW_BODY="$1" \
    PATH="$enqueue_gh:$PATH" bash "$root/scripts/pr-ready-audit.sh" --enqueue 999 2>&1)" \
    || status=$?
  printf '%s|%s|%s' "$status" "$(tr '\n' ' ' <<< "$out")" \
    "$(grep -c . "$tmp/merge-calls.log" || true)"
}
enqueue_pass() {  # enqueue_pass: the clean verdict this harness's pull request has earned
  printf '{"reviewed_sha":"%s","base_sha":"%s","verdict":"PASS","findings":[]}' \
    "$enqueue_head" "$enqueue_base"
}
enqueue_blocking() {  # enqueue_blocking: the blocking verdict the two witnesses below hide
  printf '{"reviewed_sha":"%s","base_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[]}' \
    "$enqueue_head" "$enqueue_base"
}
# THE CONTROL FIRST, and it is the only reason a zero below is evidence: the same pull request with
# the same clean verdict and nothing hidden anywhere is READY, is enqueued, and the recorder holds
# exactly one call.
{ printf 'Reviewed head: %s\n\n```json\n' "$enqueue_head"; enqueue_pass; printf '\n```\n'; } \
  > "$tmp/enqueue-clean.md"
got="$(merge_run "$tmp/enqueue-clean.md")"
contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING merge control" "$got" "enqueued #999"
expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING merge control calls" "${got##*|}" 1
expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING merge control status" "${got%%|*}" 0
# Round one's first witness, and the same comment with the hidden fence's language written plainly:
# the encoded spelling buys nothing the literal one does not already get.
entity_witness() {  # entity_witness INFO: round one's blockquote witness, hidden fence tagged INFO
  printf 'Reviewed head: %s\n\n<!--\n```text\n-->\n\n> ```%s\n> ' "$enqueue_head" "$1"
  enqueue_blocking; printf '\n> ```\n\n<!--\n```\n-->\n\n<!--\n```json\n'
  enqueue_pass; printf '\n```\n'
}
entity_witness 'jso&#110;' > "$tmp/round1-entity.md"
entity_witness 'json'      > "$tmp/round1-entity-literal.md"
# Round one's second witness, and its literal twin: the object's first key is `role_understanding`
# to `json.loads` either way, and the candidate count it emptied was the road to the prose parser.
key_witness() {  # key_witness KEY: round one's prose witness, the object's first key spelled KEY
  printf '<!-- upstroke-frontier-review pr=286 head=%s -->\n\n```text\n' "$enqueue_head"
  printf '{"%s":"x","reviewed_sha":"%s","verdict":"CHANGES_REQUIRED","findings":[]}' \
    "$1" "$enqueue_head"
  printf '\n```\n\nVERDICT: PASS\n'
}
key_witness 'role_understandin\u0067' > "$tmp/round1-key.md"
key_witness 'role_understanding'      > "$tmp/round1-key-literal.md"
for shape in entity entity-literal key key-literal; do
  expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape] parser" \
    "$(review_rows "$tmp/round1-$shape.md")" '1|'
  got="$(merge_run "$tmp/round1-$shape.md")"
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape]" "$got" "NOT-READY"
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape]" "$got" "review-parse-failed"
  expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape] merge calls" "${got##*|}" 0
  [[ "$got" == *"verdict=PASS"* ]] \
    && error "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape]: the audit read PASS out of it"
  [[ "$got" == *"enqueued #999"* ]] \
    && error "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape]: the audit enqueued it"
done
# AND EACH REFUSED FOR ITS OWN CASE'S REASON, so a parse that fails because some other rule moved
# is not mistaken for this one holding.
for shape in entity entity-literal; do
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape]" \
    "$(parse_why "$tmp/round1-$shape.md")" "inside the block"
done
for shape in key key-literal; do
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round1-$shape]" \
    "$(parse_why "$tmp/round1-$shape.md")" 'a verdict object inside the block'
done
# ROUND THREE'S RETURN, and why a tag's language is one function of it rather than two readings.
# That round split what the comment WROTE at one class and what a reference RESOLVED TO at a
# narrower one, so a tag holding a reference AND a character only the first class held was `json`
# to neither: ```` ```jso&#110; ```` then a literal U+0085 and `x` is `language-json` to
# `markdown-it-py` 3.0.0, which resolves the reference and splits the whole result with
# `str.split()`, and that round's parser read the swallowed blocking verdict past it -- exit 0,
# READY, one merge call. Literal U+001C did the same.
entity_witness "jso&#110;$(printf '\302\205')x" > "$tmp/round3-nel.md"
entity_witness "jso&#110;$(printf '\034')x"     > "$tmp/round3-fs.md"
# AND THE DIGIT BOUND IS A RENDERER'S, NOT THE SPECIFICATION'S. CommonMark's grammar stops a numeric
# reference at seven decimal digits and six hex; `markdown-it-py` 3.0.0 and cmark-gfm 0.29.0.gfm.6
# both resolve eight, so each tag below is the swallowed `language-json` block to both of them, and
# the parser of each of the three rounds before this one read past it -- exit 0, READY, one merge
# call.
entity_witness 'jso&#00000110;'  > "$tmp/round3-decimal8.md"
entity_witness '&#x0000006A;son' > "$tmp/round3-hex8.md"
for shape in nel fs decimal8 hex8; do
  expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round3-$shape] parser" \
    "$(review_rows "$tmp/round3-$shape.md")" '1|'
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round3-$shape]" \
    "$(parse_why "$tmp/round3-$shape.md")" "inside the block"
  got="$(merge_run "$tmp/round3-$shape.md")"
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round3-$shape]" "$got" "NOT-READY"
  contains "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round3-$shape]" "$got" "review-parse-failed"
  expect "MUT-GUARD-READS-THE-WRITTEN-SPELLING [round3-$shape] merge calls" "${got##*|}" 0
done
# MUT-NAME-FOUND-IN-NO-RENDERING. The same pull request, the same clean verdict, and an ordinary
# example block in front of it -- the shape a reviewer writes when they show what a review looks
# like. Each tag below names `json` to NO renderer, so each of these comments must be read, and
# reach READY, and be enqueued, exactly as the control was.
example_tagged() {  # example_tagged INFO: an example block tagged INFO, then the real verdict
  printf 'Reviewed head: %s\n\n```%s\nnot a review object\n```\n\n```json\n' "$enqueue_head" "$1"
  enqueue_pass; printf '\n```\n'
}
example_tagged 'json&#133;x'     > "$tmp/no-rendering-nel.md"
example_tagged 'json&amp;#133;x' > "$tmp/no-rendering-ampersand.md"
example_tagged 'json&#x1c;x'     > "$tmp/no-rendering-hex.md"
example_tagged 'text'            > "$tmp/no-rendering-plain.md"
# `json&#11;x` is round three's return: `markdown-it-py` 3.0.0 renders it `language-json&#11;x`,
# cmark-gfm 0.29.0.gfm.6 keeps the U+000B it resolves inside the name, and that round refused it.
# And nine digits is past the bound both renderers resolve, so that reference stays written too.
example_tagged 'json&#11;x'      > "$tmp/no-rendering-vt.md"
example_tagged 'jso&#000000110;' > "$tmp/no-rendering-nine.md"
for shape in nel ampersand hex plain vt nine; do
  expect "MUT-NAME-FOUND-IN-NO-RENDERING [$shape] parser" \
    "$(review_rows "$tmp/no-rendering-$shape.md")" \
    "0|json/$enqueue_head/PASS/$enqueue_base/-"
  got="$(merge_run "$tmp/no-rendering-$shape.md")"
  contains "MUT-NAME-FOUND-IN-NO-RENDERING [$shape]" "$got" "enqueued #999"
  expect "MUT-NAME-FOUND-IN-NO-RENDERING [$shape] merge calls" "${got##*|}" 1
  [[ "$got" == *NOT-READY* ]] \
    && error "MUT-NAME-FOUND-IN-NO-RENDERING [$shape]: a valid review was refused: [$got]"
done
# AND THE RULE IS NOT "STOP RESOLVING REFERENCES", which is what a repair that deleted the
# resolution would look like from here. A reference that DOES spell the name, and one that merely
# ends the word where a renderer ends it, are both a second candidate and both refuse -- measured
# against `markdown-it-py` 3.0.0, which renders each of these two tags `language-json`.
example_tagged 'jso&#110;'      > "$tmp/no-rendering-named.md"
example_tagged 'json&#32;extra' > "$tmp/no-rendering-space.md"
for shape in named space; do
  expect "MUT-NAME-FOUND-IN-NO-RENDERING [$shape] parser" \
    "$(review_rows "$tmp/no-rendering-$shape.md")" '1|'
  contains "MUT-NAME-FOUND-IN-NO-RENDERING [$shape]" \
    "$(parse_why "$tmp/no-rendering-$shape.md")" "2 places a verdict could be read from"
  got="$(merge_run "$tmp/no-rendering-$shape.md")"
  contains "MUT-NAME-FOUND-IN-NO-RENDERING [$shape]" "$got" "NOT-READY"
  expect "MUT-NAME-FOUND-IN-NO-RENDERING [$shape] merge calls" "${got##*|}" 0
done
# THE BOUNDARY ITSELF, code point by code point, so what this rule turns on is asserted rather than
# inferred from two examples of it. The six below are every code point `str.split()` ends a word at
# that `markdown-it-py` 3.0.0 will not resolve a reference to -- it leaves a reference to a control
# character written -- so spelled as a REFERENCE each is the whole of the defect: no word breaks
# there, and the tag names no language. cmark-gfm 0.29.0.gfm.6 resolves all six and keeps the
# character inside the name, so neither renderer names one of these tags `json`.
for point in 000b 001c 001d 001e 001f 0085; do
  example_tagged "json&#x$point;x" > "$tmp/no-rendering-point.md"
  expect "MUT-NAME-FOUND-IN-NO-RENDERING boundary [U+$point]" \
    "$(review_rows "$tmp/no-rendering-point.md")" \
    "0|json/$enqueue_head/PASS/$enqueue_base/-"
done
# And every one a renderer really does end a word at still ends one, so the class was narrowed and
# not emptied: tab, which both renderers end the word at, and form feed and three Unicode spaces,
# which `markdown-it-py` 3.0.0 ends it at and cmark-gfm 0.29.0.gfm.6 does not. A refusal is owed
# wherever either renderer names the tag `json`.
for point in 0009 000c 00a0 2002 3000; do
  example_tagged "json&#x$point;x" > "$tmp/no-rendering-point.md"
  expect "MUT-NAME-FOUND-IN-NO-RENDERING boundary [U+$point]" \
    "$(review_rows "$tmp/no-rendering-point.md")" '1|'
done
# AND THE SAME CODE POINTS WRITTEN LITERALLY ARE MATERIAL. `markdown-it-py` 3.0.0 splits what it
# resolved with `str.split()`, and a character the comment wrote is in that string whether or not a
# reference beside it was resolved, so a literal U+0085 after `json` DOES end the word there and
# that block is the `language-json` one a reader sees -- while `&#133;`, the same code point, is a
# reference it never resolves. Same code point, two answers, and one function gives both. All SIX
# of the loop above, so the two halves of that sentence run the same set: U+000B is the one round
# three read the other way round, refusing the reference and reading the literal as a name.
for bytes in '\013' '\302\205' '\034' '\035' '\036' '\037'; do
  example_tagged "json$(printf "$bytes")x" > "$tmp/no-rendering-point.md"
  expect "MUT-NAME-FOUND-IN-NO-RENDERING literal boundary [$bytes]" \
    "$(review_rows "$tmp/no-rendering-point.md")" '1|'
  contains "MUT-NAME-FOUND-IN-NO-RENDERING literal boundary [$bytes]" \
    "$(parse_why "$tmp/no-rendering-point.md")" "2 places a verdict could be read from"
done

# --- the frontier form: prose ------------------------------------------------------------------
cat > "$tmp/prose.md" <<'EOF'
<!-- upstroke-frontier-review pr=145 head=c3a6665000000000000000000000000000000003 -->
## Frontier review of `c3a6665` (gpt-5.6-sol, max effort)

**VERDICT: PASS**

<details>
1. **P2 — The claimed closure fails.** Detail.

2. **P3 — A scope claim does not match the diff.** Detail.

### P1 — data loss written as a heading, which a numbered-only parser would miss.

I found no MUST deviation.

VERDICT: CHANGES_REQUIRED
</details>
EOF
expect "MUT-PROSE-HEADING-FINDING/MUT-PROSE-LAST-VERDICT/MUT-QUOTED-JSON-IS-JSON" \
  "$(review_rows "$tmp/prose.md")" \
  '0|prose/c3a6665000000000000000000000000000000003/CHANGES_REQUIRED/-/MUST/P1;P2:-:0;P3:-:0'

# A prose review that quotes a JSON object stays prose: the fence has to open a line of its own,
# and the comment says which form it is in by carrying the prose form's marker.
printf '<!-- upstroke-frontier-review pr=1 -->\nReviewed head: %s\nThe object {"verdict":"PASS","findings":[]} is an example.\nVERDICT: CHANGES_REQUIRED\n' \
  "4ad962f000000000000000000000000000000001" > "$tmp/quoted.md"
expect MUT-QUOTED-JSON-IS-JSON "$(review_rows "$tmp/quoted.md")" \
  '0|prose/4ad962f000000000000000000000000000000001/CHANGES_REQUIRED/-/-'

# No field is ever empty, and no field is ever missing. `-` is how the parser says "the review did
# not record this"; a tab-separated protocol folded an empty field and shifted the verdict into the
# commit's column, so the audit reported a reviewed commit of `PASS`.
printf 'Reviewed head: %s\n\n```json\n{"verdict":"PASS","findings":[]}\n```\n' \
  4ad962f000000000000000000000000000000001 > "$tmp/no-sha.md"
expect MUT-META-FIELD-COLLAPSE "$(review_rows "$tmp/no-sha.md")" '0|json/-/PASS/-/-'
expect MUT-META-FIELD-COLLAPSE "$(parse_nul review "$tmp/no-sha.md")" '0|review|json|-|PASS|-|-|0|'
printf '<!-- upstroke-frontier-review pr=1 -->\nno reviewed head anywhere\n' > "$tmp/no-head.md"
expect MUT-META-FIELD-COLLAPSE "$(review_rows "$tmp/no-head.md")" '0|prose/-/-/-/-'
# A verdict block holding JSON that is not a verdict object is a COMPLETE description of a review
# that records nothing -- one finding the audit cannot judge, and every field absent. It is a
# result rather than a refusal because the audit blocks on every `ERR` row it reads, and "there is
# a review and it says nothing readable" is what a person has to be told.
printf '```json\n[]\n```\n' > "$tmp/not-an-object.md"
expect MUT-BAD-SEVERITY-PASSES "$(review_rows "$tmp/not-an-object.md")" '0|json/-/-/-/-;ERR:unparsed:0'
got="$(STUB_REVIEW_BODY="$tmp/not-an-object.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-BAD-SEVERITY-PASSES "$got" "NOT-READY"
contains MUT-BAD-SEVERITY-PASSES "$got" "findings-unparsed:unparsed"

# A review string that carries a separator writes no record. The base is not a commit and is
# refused as one -- not trimmed to the part of it that is -- and the finding survives.
printf 'Reviewed head: %s\n\n```json\n{"reviewed_sha":"%s","base_sha":"%s\\nEND\\t-\\t0","verdict":"PASS","findings":[{"id":"A-DEFERRABLE","severity":"P3"}]}\n```\n' \
  4ad962f000000000000000000000000000000001 4ad962f000000000000000000000000000000001 \
  5157509000000000000000000000000000000002 > "$tmp/forged-base.md"
expect MUT-REVIEW-FORGES-PROTOCOL "$(review_rows "$tmp/forged-base.md")" \
  '0|json/4ad962f000000000000000000000000000000001/PASS/-/-;P3:A-DEFERRABLE:0'
# A finding id is a field of the same protocol and is held to the same rule. It is refused rather
# than trimmed to "no id": no id is a MANUAL line for a person, and this is an id that wrote rows.
printf '```json\n{"verdict":"PASS","findings":[{"id":"A\\tEND\\t-\\t0","severity":"P3"}]}\n```\n' \
  > "$tmp/forged-id.md"
expect MUT-REVIEW-FORGES-PROTOCOL "$(review_rows "$tmp/forged-id.md")" '0|json/-/PASS/-/-;ERR:bad-id:0'
# And the separator this protocol actually uses. A NUL cannot appear in a field the parser emits;
# where the review puts one there, the parse REFUSES rather than emitting a field that would make
# a record boundary of its own.
"$parser_python" - "$tmp/nul-verdict.md" <<'PY'
import sys
open(sys.argv[1], "wb").write(
    b"<!-- upstroke-frontier-review pr=1 head=c3a6665000000000000000000000000000000003 -->"
    b"\n\nVERDICT: PA\x00SS\n"
)
PY
got="$(parse_nul review "$tmp/nul-verdict.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-REVIEW-FORGES-PROTOCOL: a field holding the record separator was emitted, got [$got]"
expect MUT-REVIEW-FORGES-PROTOCOL "${got#*|}" ''

# --- the parse is whole or it is nothing --------------------------------------------------------
# THE FINDING THIS ROUND WAS WRITTEN FOR. The old parser printed one record per finding and
# checked none of those writes: with `write()` made to return EIO on the row carrying a P1, the
# function around it still returned 0, `END` printed after the gap, and a review that blocks
# audited as a clean PASS -- READY, and a merge call. There is one write now, it is confirmed
# before the exit status is decided, and the destination is renamed into place only after the
# bytes are on the device. `/dev/full` fails every write on any machine, with no ptrace needed;
# the pull request carries the `strace`-injected EIO as well.
printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n1. **P1 - a thing that blocks.** Detail.\n\nVERDICT: PASS\n' \
  c3a6665000000000000000000000000000000003 > "$tmp/prose-numbered-p1.md"
expect MUT-PARSE-WRITE-UNCHECKED "$(review_rows "$tmp/prose-numbered-p1.md")" \
  '0|prose/c3a6665000000000000000000000000000000003/PASS/-/-;P1:-:0'
# `--out`, which is how the audit runs it, with the write denied by `ulimit -f 0` -- a real
# `write()` that fails, on any machine, with no ptrace needed. Two assertions, and the second is
# the one the finding was about: the status says the parse failed, AND THE DESTINATION IS NOT
# WRITTEN, so there is no half a result for a caller to read even if it ignored the status.
: > "$tmp/denied.out"
write_status=0
( trap '' XFSZ; ulimit -f 0
  "$parser_python" scripts/pr-review-parse.py review --nul --out "$tmp/denied.out" \
    "$tmp/prose-numbered-p1.md" 2>/dev/null ) || write_status=$?
((write_status != 0)) \
  || error "MUT-PARSE-WRITE-UNCHECKED: a parse whose result could not be written reported success"
expect MUT-PARSE-WRITE-UNCHECKED "$(wc -c < "$tmp/denied.out")" 0
expect MUT-PARSE-WRITE-UNCHECKED "$(ls "$tmp/denied.out".* 2>/dev/null | wc -l)" 0
# The same call with the write allowed writes the whole payload, so the case above is about the
# denied write and not about the fixture.
expect MUT-PARSE-WRITE-UNCHECKED "$(parse_nul review "$tmp/prose-numbered-p1.md" "$tmp/denied.out")" \
  '0|review|prose|c3a6665000000000000000000000000000000003|PASS|-|-|1|P1|-|0|'
# --- the staging file belongs to the invocation that created it ---------------------------------
# `OUT + ".part"` is ONE PATHNAME SHARED BY EVERY INVOCATION writing to one destination, and two
# of them overlapping publishes one's bytes under the other's exit 0: A flushed a blocking review
# and paused in `fsync`, B truncated the same staging file and wrote a clean `PASS` over it, A woke
# and renamed B's bytes into place -- A exit 0 publishing PASS with no findings, from an input
# carrying CHANGES_REQUIRED and a P1; B then exit 1, its staging pathname gone. Standards section 8
# says it directly: a unique staging path, never a fixed temporary name concurrent writers collide
# on. The pull request carries the `strace`-timed race; this is the same property without timing.
#
# A DIRECTORY standing at the fixed pathname is the whole case: a parse that still reaches for
# `OUT.part` cannot open it and exits non-zero, and one that creates its own staging name never
# looks. It needs no permission trick, so it is the same case for a run as root.
mkdir -p "$tmp/stage"
staged_out="$tmp/stage/payload.nul"
mkdir -p "$staged_out.part"
expect MUT-STAGING-PATH-SHARED "$(parse_nul review "$tmp/prose-numbered-p1.md" "$staged_out")" \
  '0|review|prose|c3a6665000000000000000000000000000000003|PASS|-|-|1|P1|-|0|'
[[ -d "$staged_out.part" ]] \
  || error "MUT-STAGING-PATH-SHARED: the parse wrote through the fixed staging pathname"
# and it leaves nothing behind beside the destination: a staging file per invocation is not a
# staging file per invocation left lying there.
expect MUT-STAGING-PATH-SHARED "$(ls "$tmp/stage" | grep -vcx 'payload.nul\|payload.nul.part')" 0
rmdir "$staged_out.part"
# The published bytes are this invocation's own, and its permissions are what a plain create
# leaves: making the staging name unique is not also a change to what the caller reads.
expect MUT-STAGING-PATH-SHARED "$(stat -c %a "$staged_out")" "$(
  : > "$tmp/stage/plain"; stat -c %a "$tmp/stage/plain")"

# And the destination the parse could not open at all: nothing is written there either.
mkdir -p "$tmp/unwritable"
: > "$tmp/unwritable/out"
chmod a-w "$tmp/unwritable"
out_status=0
"$parser_python" scripts/pr-review-parse.py review --nul --out "$tmp/unwritable/out" \
  "$tmp/prose-numbered-p1.md" 2>/dev/null || out_status=$?
chmod u+w "$tmp/unwritable"
if ((EUID != 0)); then    # root writes into a directory with no write bit, so the case is not one
  ((out_status != 0)) \
    || error "MUT-PARSE-WRITE-UNCHECKED: a parse that could not open its destination reported success"
  expect MUT-PARSE-WRITE-UNCHECKED "$(wc -c < "$tmp/unwritable/out")" 0
fi
# The rendering the gate itself reads, to stdout, held to the same rule.
write_status=0
"$parser_python" scripts/pr-review-parse.py review --nul "$tmp/prose-numbered-p1.md" \
  > /dev/full 2>/dev/null || write_status=$?
((write_status != 0)) \
  || error "MUT-PARSE-WRITE-UNCHECKED: a parse whose result went nowhere reported success"

# --- a write that returned a smaller number is not a write ---------------------------------------
# `write()` RETURNS A BYTE COUNT AND IT IS NOT ALWAYS THE WHOLE PAYLOAD. Under `PYTHONUNBUFFERED=1`
# `sys.stdout.buffer` is the raw file object, whose `write` does one `write(2)` and hands back what
# the kernel took. With the process's `RLIMIT_FSIZE` at 1,024 bytes, stdout on a file and a review
# of 499 numbered findings rendering to 3,560 bytes, `strace` recorded
# `write(1, ..., 3560) = 1024` and then `+++ exited with 0 +++`: the count discarded, a flush of an
# unbuffered stream finishing nothing, and a findings list 2,536 bytes short of itself announced as
# whole -- in both renderings, and with the record count the shell checks still declaring every
# record the result was built with. That is the seventh round's unchecked write in the one place
# that was left.
{ printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n' c3a6665000000000000000000000000000000003
  for ((n = 1; n <= 499; n++)); do printf '%d. **P1 - a thing that blocks.** Detail.\n\n' "$n"; done
  printf 'VERDICT: PASS\n'; } > "$tmp/many-findings.md"
for rendering in --nul --json; do
  # the control first: with no limit the whole payload is written and the parse exits 0
  whole_status=0
  "$parser_python" scripts/pr-review-parse.py review "$rendering" "$tmp/many-findings.md" \
    > "$tmp/many.whole" 2>/dev/null || whole_status=$?
  expect "MUT-PARSE-SHORT-WRITE [$rendering] control" "$whole_status" 0
  whole="$(wc -c < "$tmp/many.whole")"
  ((whole > 1024)) \
    || error "MUT-PARSE-SHORT-WRITE [$rendering]: the fixture renders to [$whole] bytes, which the limit would not cut"
  short_status=0
  ( trap '' XFSZ; ulimit -f 1
    PYTHONUNBUFFERED=1 "$parser_python" scripts/pr-review-parse.py review "$rendering" \
      "$tmp/many-findings.md" > "$tmp/many.short" 2>/dev/null ) || short_status=$?
  ((short_status != 0)) \
    || error "MUT-PARSE-SHORT-WRITE [$rendering]: a result whose write stopped part way reported success"
  # and the write really did stop part way, so the case is about the count and not about a refusal
  # somewhere else: what landed is some of the payload and not all of it.
  landed="$(wc -c < "$tmp/many.short")"
  ((landed > 0 && landed < whole)) \
    || error "MUT-PARSE-SHORT-WRITE [$rendering]: no short write happened, [$landed] of [$whole] bytes"
done
# The class, not the site. The only `write` call in the parser is the one inside `write_all`, which
# reads what it returns, and the stderr diagnostic, which is not the result channel: any other is a
# count nobody read. Asked of the SYNTAX TREE, so a sentence in a comment cannot answer for code.
expect MUT-PARSE-SHORT-WRITE "$("$parser_python" - scripts/pr-review-parse.py <<'WRITESHAPE'
import ast, sys


def dotted(node):
    parts = []
    while isinstance(node, ast.Attribute):
        parts.append(node.attr)
        node = node.value
    if isinstance(node, ast.Name):
        parts.append(node.id)
    return ".".join(reversed(parts))


loose = 0
for node in ast.walk(ast.parse(open(sys.argv[1]).read())):
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) \
            and node.func.attr == "write" \
            and dotted(node.func) not in ("stream.write", "sys.stderr.buffer.write"):
        loose += 1
print(loose)
WRITESHAPE
)" 0
# An input the parse cannot read is not an empty review, and it is not a review in the other
# format either: FORMAT DETECTION IS PART OF THE PARSE. `if grep ...; then json; else prose; fi`
# made a file it could not read into "prose" and CHOSE A PARSER on it -- and the prose parser
# reads a JSON review's `VERDICT:` line from outside its verdict object and misses every severity
# the object spells with a JSON escape (`"P\u0031"` holds no `P1` to find; the unescaped `"P1"`
# does, which is why the fixture below writes the escape).
printf '```json\n{"verdict":"CHANGES_REQUIRED","findings":[{"id":"CRITICAL","severity":"P\\u0031"}]}\n```\n' \
  > "$tmp/escaped-severity.md"
expect MUT-REVIEW-KIND-UNREADABLE-IS-PROSE "$(review_rows "$tmp/escaped-severity.md")" \
  '0|json/-/CHANGES_REQUIRED/-/-;P1:CRITICAL:0'
# The same review with a `VERDICT: PASS` line written under it is a comment that says both things,
# and the prose reading of it is the clean PASS with no findings -- the escape hides the P1 from
# the stray scan. It is not read either way.
{ cat "$tmp/escaped-severity.md"; printf '\nVERDICT: PASS\n'; } > "$tmp/escaped-severity-both.md"
got="$(review_rows "$tmp/escaped-severity-both.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-REVIEW-KIND-UNREADABLE-IS-PROSE: a comment saying CHANGES_REQUIRED and PASS parsed, got [$got]"
[[ "$got" == *PASS* ]] \
  && error "MUT-REVIEW-KIND-UNREADABLE-IS-PROSE: the prose PASS was taken over the object, got [$got]"
#
# It is also where MUT-PROSE-READ-SUPPRESSED now lives. That case was a read INSIDE the prose
# parser whose failure was reported as "nothing matched", with the completeness marker printed
# over it: a review carrying a P3 under a PASS parsed as a clean PASS with no findings. There is
# no read inside the parse to suppress a status on any more -- the input is read once, whole, and
# a read that failed is a parse that failed, which is these cases.
for unreadable in "$tmp/no-such-review.md" "$tmp"; do
  got="$(review_rows "$unreadable")"
  [[ "$got" == 0\|* ]] \
    && error "MUT-REVIEW-KIND-UNREADABLE-IS-PROSE/MUT-PROSE-READ-SUPPRESSED: an input the parse could not read was given a format, got [$got]"
done
# Bytes that are not UTF-8 are not text with the bad bytes dropped, for the same reason.
printf '```json\n{"verdict":"PASS","findings":[]}\n```\n\xff\xfe\n' > "$tmp/not-utf8.md"
got="$(review_rows "$tmp/not-utf8.md")"
[[ "$got" == 0\|* ]] \
  && error "MUT-REVIEW-KIND-UNREADABLE-IS-PROSE/MUT-PROSE-READ-SUPPRESSED: bytes that are not UTF-8 parsed as a review, got [$got]"
# Through main, because the helper refusing is only half of it: the audit must block on the status
# rather than reading whatever the file holds.
got="$(STUB_REVIEW_BODY="$tmp/not-utf8.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-REVIEW-KIND-UNREADABLE-IS-PROSE "$got" "review-parse-failed"
contains MUT-REVIEW-KIND-UNREADABLE-IS-PROSE "$got" "NOT-READY"
[[ "$got" == *"verdict=PASS"* ]] \
  && error "MUT-REVIEW-KIND-UNREADABLE-IS-PROSE: a verdict was read out of a review the parser refused"
# The same review readable is judged by the JSON parser and blocks on its P1, so the case above is
# about the refusal and not about the review.
got="$(STUB_REVIEW_BODY="$tmp/escaped-severity.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
contains MUT-REVIEW-KIND-UNREADABLE-IS-PROSE "$got" "open-P1:CRITICAL"
[[ "$got" == *review-parse-failed* ]] \
  && error "MUT-REVIEW-KIND-UNREADABLE-IS-PROSE: a readable review was reported unreadable"

# A PAYLOAD THAT STOPPED ARRIVING IS NOT A SHORTER FINDINGS LIST. `read` ends a loop at end of
# input and at a failed read alike, so the audit cannot tell the two apart by looking; it checks
# what arrived against the record count the parser computed. Truncating the payload after the
# first finding is what a short read looks like from inside the loop.
"$parser_python" scripts/pr-review-parse.py review --nul --out "$tmp/short.payload" "$tmp/json.md"
truncate -s 60 "$tmp/short.payload"
short_status=0
( fields=(); read_parser_fields "$tmp/short.payload" review 7 3 ) || short_status=$?
((short_status != 0)) \
  || error "MUT-REVIEW-PARSE-TRUNCATED: a payload that stopped arriving was read as a whole one"
# The whole payload passes the same check, so the case above is about the truncation.
expect MUT-REVIEW-PARSE-TRUNCATED "$(
  "$parser_python" scripts/pr-review-parse.py review --nul --out "$tmp/whole.payload" "$tmp/json.md"
  fields=(); read_parser_fields "$tmp/whole.payload" review 7 3 && echo "${#fields[@]}"
)" 22
# A payload of another kind, and one whose declared count does not match what it carries, are both
# refused: the tag and the count are checked before a field is read as anything.
printf 'ledger\0' > "$tmp/wrong-tag.payload"
( fields=(); read_parser_fields "$tmp/wrong-tag.payload" review 7 3 ) \
  && error "MUT-REVIEW-PARSE-TRUNCATED: a payload of the wrong kind was read as a review"
printf 'review\0json\0-\0PASS\0-\0-\09\0' > "$tmp/lying-count.payload"
( fields=(); read_parser_fields "$tmp/lying-count.payload" review 7 3 ) \
  && error "MUT-REVIEW-PARSE-TRUNCATED: a payload declaring more records than it carries was read"

# --- a check that failed is the answer, not the input to another attempt --------------------------
# A verdict is a whole token, in every locale. `[A-Z_]+$` matches the TAIL of a dirty token, and
# `[A-Z_]` inside a grep is whatever the locale's collating order says it is: under en_US.utf8 the
# grep carried `FAIL\xc3\x89PASS` through and the check took the clean `PASS` off its end, so a
# review that says FAIL approved the change -- while the same file under `C` blocked. The parser is
# a program now and its character classes are code-point ranges, which mean the same thing
# everywhere; the token that is not a verdict is still carried whole, `VERDICT:` and all, so the
# blocker names it.
printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\nVERDICT: FAIL\xc3\x89PASS\n' \
  c3a6665000000000000000000000000000000003 > "$tmp/prose-suffix.md"
suffix_want="$(printf '0|prose/c3a6665000000000000000000000000000000003/VERDICT: FAIL\xc3\x89PASS/-/-')"
expect MUT-PROSE-VERDICT-SUFFIX "$(review_rows "$tmp/prose-suffix.md")" "$suffix_want"
for loc in "${locales[@]}"; do
  got="$(LC_ALL="$loc" review_rows "$tmp/prose-suffix.md")"
  expect "MUT-PROSE-VERDICT-SUFFIX under $loc" "$got" "$suffix_want"
done
# A verdict wrapped in the emphasis a frontier review writes is still that verdict, and the last
# line still wins: the whole-token rule must not refuse the form the reviews actually use.
printf '<!-- upstroke-frontier-review pr=1 -->\nhead=%s\n**VERDICT: CHANGES_REQUIRED**\n**VERDICT: PASS**\n' \
  c3a6665000000000000000000000000000000003 > "$tmp/prose-emphasis.md"
expect MUT-PROSE-VERDICT-SUFFIX "$(review_rows "$tmp/prose-emphasis.md")" \
  '0|prose/c3a6665000000000000000000000000000000003/PASS/-/-'
# The head is the FIRST marker, and a first marker that does not read whole is not a licence to
# take the second: the audit would then check the pull request's head against a commit the review
# never named. `-` blocks; the later line does not stand in for it.
printf '<!-- upstroke-frontier-review pr=1 -->\nhead=c3a6665\xc3\x8900000000000000000000000000000003\nReviewed head: %s\n' \
  4ad962f000000000000000000000000000000001 > "$tmp/prose-head.md"
for loc in "${locales[@]}"; do
  got="$(LC_ALL="$loc" review_rows "$tmp/prose-head.md")"
  expect "MUT-PROSE-HEAD-NOT-FIRST under $loc" "$got" '0|prose/-/-/-/-'
done
# The whole-token check refuses `VERDICT: ::PASS`, and the branch it fell into used to strip the
# leading colons and asterisks off what it had just refused -- so the rejecting branch minted the
# `PASS` the check exists to withhold. Nothing is stripped: the whole matched run stands, and
# because every one of these begins with `VERDICT:` it cannot be the token the audit lets through.
for junk in ': ::PASS' ': :PASS' ': **PASS' ': *PASS'; do
  printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\nVERDICT%s\n' \
    c3a6665000000000000000000000000000000003 "$junk" > "$tmp/prose-salvage.md"
  for loc in ambient "${locales[@]}"; do
    if [[ "$loc" == ambient ]]; then got="$(review_rows "$tmp/prose-salvage.md")"
    else got="$(LC_ALL="$loc" review_rows "$tmp/prose-salvage.md")"; fi
    [[ "$got" == *'/PASS/'* ]] \
      && error "MUT-PROSE-VERDICT-SALVAGE under $loc: [VERDICT$junk] was salvaged into PASS, got [$got]"
    contains MUT-PROSE-VERDICT-SALVAGE "$got" '/VERDICT'
  done
done

# --- a review too big for a pipe buffer ---------------------------------------------------------
# `<<<` is a here-document, and bash writes one to a TEMPORARY FILE once it outgrows a pipe
# buffer; a temporary file it cannot create is a redirection that failed, so the command never
# runs and the shell returns 1 -- `grep`'s "no match" -- and a compound command fed by one is
# skipped outright, with a status of 0 that neither a captured status nor `set -e` can see. Both
# shapes were live in the prose parser: one lost the only `P1` in a 40,000-character review and
# printed `END` over it, the other would have dropped a findings list whole. There is no
# here-string in the parse path at all now, and `ulimit -f` denies the temporary file these used
# to need, so a parser that still wanted one would be caught here.
{ printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\nVERDICT: PASS\n\n' \
    c3a6665000000000000000000000000000000003
  for _ in $(seq 1 400); do printf '\xc3\xa9%.0s' $(seq 1 100); printf '\n'; done
  printf '\nA standalone P1 in running text.\n'; } > "$tmp/prose-big-stray.md"
big_want='0|prose/c3a6665000000000000000000000000000000003/PASS/-/P1'
expect MUT-HERESTRING-FAILURE-AS-ANSWER "$(review_rows "$tmp/prose-big-stray.md")" "$big_want"
expect MUT-HERESTRING-FAILURE-AS-ANSWER \
  "$(trap '' XFSZ; ulimit -f 1; review_rows "$tmp/prose-big-stray.md")" "$big_want"
{ printf '<!-- upstroke-frontier-review pr=232 head=%s -->\n\n' \
    c3a6665000000000000000000000000000000003
  for i in $(seq 1 8000); do printf '%d. **P3 - a deferrable thing.** Detail.\n' "$i"; done
  printf '\nVERDICT: PASS\n'; } > "$tmp/prose-many-findings.md"
expect MUT-HERESTRING-FAILURE-AS-ANSWER \
  "$(trap '' XFSZ; ulimit -f 1; review_rows "$tmp/prose-many-findings.md" | grep -o 'P3:-:0' | wc -l)" 8000
expect MUT-HERESTRING-FAILURE-AS-ANSWER \
  "$(review_rows "$tmp/prose-many-findings.md" | grep -o 'P3:-:0' | wc -l)" 8000

# --- the frontmatter id match ------------------------------------------------------------------
printf -- '---\nid: OTHER-ID\nseverity: P2\n---\n\nThe prose below repeats a line.\nid: TARGET-ID\n' > "$tmp/prose-id.md"
printf -- '---\nid: TARGET-ID\nseverity: P2\n---\n\nBody.\n' > "$tmp/front-id.md"
printf -- '---\nid: TARGETXID\n---\n' > "$tmp/x-id.md"
printf 'id: TARGET-ID\n---\nno opening fence\n' > "$tmp/no-front.md"
# The answer is what it PRINTS -- `1` or `0` -- and its status is only ever failure. `cmd < file`
# on a file bash cannot open returns 1 with the command never run, and 1 used to be this helper's
# word for "read to the end and did not carry the id", so an input that could not be opened
# answered in the matcher's place. Both are checked on every case: the answer, and a status of 0
# saying the matcher is the one who gave it.
has_id() {  # has_id ID FILE: "<status>|<answer>"
  local out status=0
  out="$(frontmatter_has_id "$1" < "$2")" || status=$?
  printf '%s|%s' "$status" "$out"
}
expect MUT-FRONTMATTER-BY-SUBSTRING "$(has_id TARGET-ID "$tmp/prose-id.md")" "0|0"   # an id in prose is not one
expect MUT-FRONTMATTER-BY-SUBSTRING "$(has_id TARGET-ID "$tmp/front-id.md")" "0|1"
expect MUT-FRONTMATTER-PATTERN "$(has_id TARGET.ID "$tmp/x-id.md")" "0|0"            # a dot is not a regex
expect MUT-FRONTMATTER-BY-SUBSTRING "$(has_id TARGET-ID "$tmp/no-front.md")" "0|0"   # no frontmatter block
# An input the redirection could not open is not a file that does not carry the id: nothing ran,
# so there is no answer, and the status says so.
expect MUT-FINDING-BLOB-OPEN-AS-MISS "$(has_id TARGET-ID "$tmp/no-such-finding-file.md" 2>/dev/null)" "1|"

# --- counting the files that file a finding ------------------------------------------------------
# The one place git is exercised here, in a repository built for it. This count decides whether a
# deferred finding is properly filed, and the shape rules above cannot see any of the ways it goes
# wrong: `git grep` exits 1 for "nothing matched" and above 1 for an error, and the names come back
# NUL-separated, which a command substitution silently drops -- leaving a loop with no separators,
# no iterations, and a count of zero for every finding in a tree that holds the file.
findings_repo="$tmp/findings-repo"
mkdir -p "$findings_repo/findings"
git init -q "$findings_repo"
printf -- '---\nid: A-DEFERRABLE\nseverity: P3\n---\n\nBody.\n' > "$findings_repo/findings/a.md"
printf -- '---\nid: B-OTHER\nseverity: P3\n---\n\nBody.\n' > "$findings_repo/findings/b with space.md"
printf -- '---\nid: C-TWICE\n---\n\nBody.\n' > "$findings_repo/findings/c1.md"
printf -- '---\nid: C-TWICE\n---\n\nBody.\n' > "$findings_repo/findings/c2.md"
printf -- 'id: D-PROSE-ONLY\nnot frontmatter\n' > "$findings_repo/findings/d.md"
# Two files filing one id, the second with a long line after the id inside its frontmatter. The
# match is on the id line, so `grep -q` exited there and the `awk` still writing the rest took
# SIGPIPE: `pipefail` reported 141, the caller read any non-zero as "this file does not carry the
# id", and the duplicate counted as no file at all. One file, and the pull request was ready.
printf -- '---\nid: E-LONG-LINE\n---\n\nBody.\n' > "$findings_repo/findings/e1.md"
{ printf -- '---\nid: E-LONG-LINE\ndescription: '
  head -c 262144 /dev/zero | tr '\0' 'x'
  printf -- '\n---\n\nBody.\n'
} > "$findings_repo/findings/e2.md"
git -C "$findings_repo" add -A
git -C "$findings_repo" -c user.email=t@example -c user.name=t commit -qm "file the findings"
count_in_fixture() { (cd "$findings_repo" && finding_file_count "$1" HEAD); }
expect MUT-FINDING-FILE-COUNT "$(count_in_fixture A-DEFERRABLE)" 1
expect MUT-FINDING-FILE-COUNT "$(count_in_fixture B-OTHER)" 1          # the name has a space in it
expect MUT-FINDING-FILE-COUNT "$(count_in_fixture C-TWICE)" 2          # two files is not one
expect MUT-FINDING-FILE-COUNT "$(count_in_fixture D-PROSE-ONLY)" 0     # the id is not in frontmatter
expect MUT-FINDING-FILE-COUNT "$(count_in_fixture NOT-FILED-ANYWHERE)" 0

# THE LEDGER'S OLD PREFIX IS STILL COUNTED, BECAUSE THE TREEISH IS THE CALLER'S REVISION. The
# ledger moved from reviews/findings/ to findings/ on 2026-09-12 (pull request #276) and
# `main` passes the pull request's own head, which for every pull request open across that move
# predates it. `git ls-tree` with a pathspec matching nothing exits 0 with no output, so the
# status check inside the count sees no failure and a finding filed once came back as 0 files --
# NOT-READY, `no-file:<id>`, with the audit exiting 0. These two cases fail if the compatibility
# prefix is dropped from that pathspec: the first is the tree as a pre-move head holds it, and the
# second is a tree carrying BOTH directories, which is a branch cut before the move that files
# under the old prefix and merges without conflict. One file is one finding under either name.
legacy_repo="$tmp/legacy-ledger-repo"
mkdir -p "$legacy_repo/reviews/findings"
git init -q "$legacy_repo"
printf -- '---\nid: LEGACY-P3\nseverity: P3\n---\n\nBody.\n' \
  > "$legacy_repo/reviews/findings/P3_correctness_202609010001_filed-before-the-move.md"
git -C "$legacy_repo" add -A
git -C "$legacy_repo" -c user.email=t@example -c user.name=t commit -qm "file a finding before the move"
expect MUT-FINDING-LEDGER-PREFIX-HISTORICAL \
  "$( (cd "$legacy_repo" && finding_file_count LEGACY-P3 HEAD) )" 1
mkdir -p "$legacy_repo/findings"
printf -- '---\nid: BOTH-PREFIXES\nseverity: P3\n---\n\nBody.\n' \
  > "$legacy_repo/findings/P3_correctness_202609010002_filed-after-the-move.md"
printf -- '---\nid: BOTH-PREFIXES\nseverity: P3\n---\n\nBody.\n' \
  > "$legacy_repo/reviews/findings/P3_correctness_202609010003_filed-before-it.md"
git -C "$legacy_repo" add -A
git -C "$legacy_repo" -c user.email=t@example -c user.name=t commit -qm "a tree carrying both directories"
expect MUT-FINDING-LEDGER-PREFIX-HISTORICAL \
  "$( (cd "$legacy_repo" && finding_file_count BOTH-PREFIXES HEAD) )" 2
expect MUT-FINDING-LEDGER-PREFIX-HISTORICAL \
  "$( (cd "$legacy_repo" && finding_file_count LEGACY-P3 HEAD) )" 1
# A tree it cannot read is not a tree with no files in it.
if (cd "$findings_repo" && finding_file_count A-DEFERRABLE deadbeefdeadbeefdeadbeefdeadbeefdeadbeef) > "$tmp/tree.out" 2>&1; then
  error "MUT-FINDING-FILE-COUNT: an unreadable tree was counted, got [$(cat "$tmp/tree.out")]"
fi
# Two files, one of which the frontmatter read cannot finish. This is the count's whole job: two
# files filing one id is `duplicate-file` and one is ready, so a file that dropped out of the
# count is the difference between blocked and merged.
expect MUT-FINDING-FILE-READ-AS-MISS "$(count_in_fixture E-LONG-LINE)" 2
# The helper's own report on that file, so the count above is not the only witness: `1` printed
# and a status of 0, from a read that went to the end of a frontmatter block a quarter of a
# megabyte long.
expect MUT-FINDING-FILE-READ-AS-MISS \
  "$(has_id E-LONG-LINE "$findings_repo/findings/e2.md")" "0|1"

# A blob the tree still names and the object store no longer holds. `git grep` cannot find this
# one for anybody: it printed `unable to read` on stderr, exited 0, and returned only the readable
# name -- so checking its status catches nothing, and the candidates have to come from the tree.
broken_repo="$tmp/broken-repo"
mkdir -p "$broken_repo/findings"
git init -q "$broken_repo"
printf -- '---\nid: F-TWICE\n---\n\nBody.\n' > "$broken_repo/findings/f1.md"
printf -- '---\nid: F-TWICE\n---\n\nAnother body.\n' > "$broken_repo/findings/f2.md"
git -C "$broken_repo" add -A
git -C "$broken_repo" -c user.email=t@example -c user.name=t commit -qm "file one id twice"
expect MUT-FINDING-FILE-READ-AS-MISS "$( (cd "$broken_repo" && finding_file_count F-TWICE HEAD) )" 2
broken_blob="$(git -C "$broken_repo" rev-parse HEAD:findings/f2.md)"
rm -f "$broken_repo/.git/objects/${broken_blob:0:2}/${broken_blob:2}"
if (cd "$broken_repo" && finding_file_count F-TWICE HEAD) > "$tmp/blob.out" 2>&1; then
  error "MUT-FINDING-FILE-READ-AS-MISS: a file whose blob could not be read was counted, got [$(cat "$tmp/blob.out")]"
fi

# A frontmatter read that DIES is not a file that does not carry the id. While the read was a
# pipeline this was unprovable from the outside: under `pipefail` the reader's 2 stood behind the
# matcher's ordinary 1 -- nothing matched, because nothing arrived -- and 1 is an answer. Two
# files filing one id came back as one, which is `duplicate-file` turning into ready.
#
# The injection is an `awk` that fails on the second file and is the real `awk` everywhere else,
# so the count has one good read and one dead one, exactly as a half-readable blob would give it.
awk_stub="$tmp/awk-stub"
mkdir -p "$awk_stub"
cat > "$awk_stub/awk" <<'STUB'
#!/usr/bin/env bash
# The frontmatter arrives on stdin, so the file is known by what is in it and not by an argument.
in="$(mktemp)"; cat > "$in"
if grep -qF 'SECOND-FILE-MARKER' "$in"; then rm -f "$in"; exit 2; fi
"$REAL_AWK" "$@" < "$in"; s=$?; rm -f "$in"; exit $s
STUB
chmod +x "$awk_stub/awk"
dup_repo="$tmp/dup-repo"
mkdir -p "$dup_repo/findings"
git init -q "$dup_repo"
printf -- '---\nid: G-TWICE\n---\n\nBody.\n' > "$dup_repo/findings/g1.md"
printf -- '---\nid: G-TWICE\nnote: SECOND-FILE-MARKER\n---\n\nBody.\n' > "$dup_repo/findings/g2.md"
git -C "$dup_repo" add -A
git -C "$dup_repo" -c user.email=t@example -c user.name=t commit -qm "file one id twice"
expect MUT-FRONTMATTER-UPSTREAM-ERROR-AS-MISS "$( (cd "$dup_repo" && finding_file_count G-TWICE HEAD) )" 2
dup_status=0
got="$(
  export REAL_AWK="$(command -v awk)" PATH="$awk_stub:$PATH"
  hash -r
  cd "$dup_repo" && finding_file_count G-TWICE HEAD 2>/dev/null
)" || dup_status=$?
((dup_status != 0)) \
  || error "MUT-FRONTMATTER-UPSTREAM-ERROR-AS-MISS: a count with a dead read in it succeeded, got [$got]"
[[ "$got" == 1 ]] \
  && error "MUT-FRONTMATTER-UPSTREAM-ERROR-AS-MISS: two files filing one id counted as one"
# And the helper itself: a read that never finished prints no answer and says so in its status.
front_status=0
front_out="$(
  export REAL_AWK="$(command -v awk)" PATH="$awk_stub:$PATH"
  hash -r
  frontmatter_has_id G-TWICE < "$dup_repo/findings/g2.md" 2>/dev/null
)" || front_status=$?
((front_status != 0)) \
  || error "MUT-FRONTMATTER-UPSTREAM-ERROR-AS-MISS: a read that died reported success"
[[ -z "$front_out" ]] \
  || error "MUT-FRONTMATTER-UPSTREAM-ERROR-AS-MISS: a read that died printed an answer [$front_out]"

# A BLOB THE CALLER COULD NOT OPEN is the same defect one layer out, and it does not need the
# matcher to fail: `frontmatter_has_id "$id" < "$blob_file"` on a file bash cannot open is a
# redirection that failed, so the shell returns 1 WITHOUT RUNNING THE MATCHER -- and 1 was the
# matcher's own word for "read to the end and did not carry the id". The count then dropped the
# file it could not open, two files filing one id came back as one, `duplicate-file` never fired
# and the pull request was READY.
#
# The injection is a `git` that is the real git everywhere except the `show` of the second file,
# whose output it writes and then REMOVES -- the blob is written, the read of it is what fails.
# Removal rather than a mode change so the case is the same case for a run as root.
git_stub="$tmp/git-stub"
mkdir -p "$git_stub"
cat > "$git_stub/git" <<'STUB'
#!/usr/bin/env bash
"$REAL_GIT" "$@"; s=$?
if [[ "$1" == show && -n "${BLOB_FILE:-}" && -f "$BLOB_FILE" ]] \
   && grep -qF 'SECOND-FILE-MARKER' "$BLOB_FILE"; then rm -f "$BLOB_FILE"; fi
exit $s
STUB
chmod +x "$git_stub/git"
# `mktemp` hands out predictable names so the stub above can name the blob file exactly;
# `finding_file_count` takes the candidate list first and the blob second.
mktemp_stub="$tmp/mktemp-stub"
mkdir -p "$mktemp_stub"
cat > "$mktemp_stub/mktemp" <<'STUB'
#!/usr/bin/env bash
(($# == 0)) || exec "$REAL_MKTEMP" "$@"
n=$(( $(cat "$MK_COUNT") + 1 )); echo "$n" > "$MK_COUNT"
p="$MK_DIR/ft.$n"; : > "$p"; echo "$p"
STUB
chmod +x "$mktemp_stub/mktemp"
mkdir -p "$tmp/ftmp"
blob_status=0
got="$(
  export REAL_GIT="$(command -v git)" REAL_MKTEMP="$(command -v mktemp)"
  export MK_COUNT="$tmp/ftmp/count" MK_DIR="$tmp/ftmp" BLOB_FILE="$tmp/ftmp/ft.2"
  echo 0 > "$MK_COUNT"
  export PATH="$git_stub:$mktemp_stub:$PATH"
  hash -r
  cd "$dup_repo" && finding_file_count G-TWICE HEAD 2>/dev/null
)" || blob_status=$?
((blob_status != 0)) \
  || error "MUT-FINDING-BLOB-OPEN-AS-MISS: a count whose blob could not be opened succeeded, got [$got]"
[[ "$got" == 1 ]] \
  && error "MUT-FINDING-BLOB-OPEN-AS-MISS: two files filing one id counted as one"

# A LISTING THAT ENDED IS NOT A LISTING THAT WAS READ. `read` returns non-zero at end of input and
# on a failed read alike, and the loop ends on either, so a list written whole and read half way
# through printed the entries that arrived as though they were all of them: one file, no
# `duplicate-file`, READY. `git ls-tree`'s status says the list was WRITTEN and cannot see it.
#
# The injection shortens the list under the loop -- the stub `git show` for the first candidate
# truncates it to its first NUL-terminated record -- so the loop reads one entry and then an end
# of input that is not the end of the list. A read that fails outright (EIO on the descriptor) is
# the same ending and is witnessed on the pull request; this is the shape a gate can inject with
# nothing but coreutils.
cat > "$git_stub/git" <<'STUB'
#!/usr/bin/env bash
"$REAL_GIT" "$@"; s=$?
if [[ "$1" == show && -n "${CAND_FILE:-}" && -f "$CAND_FILE" ]]; then
  keep="$(tr '\0' '\n' < "$CAND_FILE" | head -1 | wc -c)"    # the first record and its separator
  truncate -s "$keep" "$CAND_FILE"
fi
exit $s
STUB
short_status=0
got="$(
  export REAL_GIT="$(command -v git)" REAL_MKTEMP="$(command -v mktemp)"
  export MK_COUNT="$tmp/ftmp/count" MK_DIR="$tmp/ftmp" CAND_FILE="$tmp/ftmp/ft.1"
  echo 0 > "$MK_COUNT"
  export PATH="$git_stub:$mktemp_stub:$PATH"
  hash -r
  cd "$dup_repo" && finding_file_count G-TWICE HEAD 2>/dev/null
)" || short_status=$?
((short_status != 0)) \
  || error "MUT-FINDING-LIST-SHORT-READ: a count over a list that stopped arriving succeeded, got [$got]"
[[ "$got" == 1 ]] \
  && error "MUT-FINDING-LIST-SHORT-READ: two files filing one id counted as one"
# The same stubs with nothing shortened still count both files, so the two cases above are about
# the reads and not about the stubs.
expect MUT-FINDING-LIST-SHORT-READ "$(
  export REAL_GIT="$(command -v git)" REAL_MKTEMP="$(command -v mktemp)"
  export MK_COUNT="$tmp/ftmp/count" MK_DIR="$tmp/ftmp"
  echo 0 > "$MK_COUNT"
  export PATH="$git_stub:$mktemp_stub:$PATH"
  hash -r
  cd "$dup_repo" && finding_file_count G-TWICE HEAD
)" 2

# WHAT GIT RECORDS DECIDES WHAT AN ENTRY IS. A committed symlink is a `120000` blob whose content
# is its target string, and `git show` hands that string over exactly as it hands over a file's
# text -- so a broken link named like a finding, pointing at `---\nid: X\n---`, filed a finding
# that had never been written. The entry is built through the index rather than with `ln -s`, so
# the case is the same one wherever this suite is run by hand.
link_repo="$tmp/link-repo"
mkdir -p "$link_repo/findings"
git init -q "$link_repo"
link_blob="$(printf -- '---\nid: SYMLINK-ID\n---\n' | git -C "$link_repo" hash-object -w --stdin)"
git -C "$link_repo" update-index --add --cacheinfo "120000,$link_blob,findings/symlink.md"
git -C "$link_repo" -c user.email=t@example -c user.name=t commit -qm "commit a link named like a finding"
expect MUT-FINDING-SYMLINK-AS-FILE "$(git -C "$link_repo" ls-tree -r HEAD -- findings/ | cut -c1-6)" 120000
expect MUT-FINDING-SYMLINK-AS-FILE "$( (cd "$link_repo" && finding_file_count SYMLINK-ID HEAD) )" 0
# The regular file beside it still counts, so the mode filter is a filter and not a refusal. The
# new file is staged BY PATH: `add -A` would see no `symlink.md` in a work tree that never had one
# and stage its deletion, and the case under test would leave the tree it is testing.
printf -- '---\nid: REGULAR-ID\n---\n\nBody.\n' > "$link_repo/findings/regular.md"
git -C "$link_repo" add -- findings/regular.md
git -C "$link_repo" -c user.email=t@example -c user.name=t commit -qm "file one finding properly"
expect MUT-FINDING-SYMLINK-AS-FILE \
  "$(git -C "$link_repo" ls-tree -r HEAD -- findings/ | cut -c1-6 | sort -u | tr '\n' ' ')" "100644 120000 "
expect MUT-FINDING-SYMLINK-AS-FILE "$( (cd "$link_repo" && finding_file_count REGULAR-ID HEAD) )" 1
expect MUT-FINDING-SYMLINK-AS-FILE "$( (cd "$link_repo" && finding_file_count SYMLINK-ID HEAD) )" 0

# --- the state comes from the blockers, and from nothing that can fail --------------------------
# THE SECOND OF THIS ROUND'S TWO P1s. `printf '%s\n' "${blockers[@]:-}" | grep -q '^manual:'` in
# the `elif` gave a manual blocker two ways to be absent: it was not there, or the scan did not
# happen. With `grep` made to exit 2 a pull request whose only blocker was
# `manual:P1-outside-the-verdict-object` printed `READY ... blockers=manual:P1-outside-the-verdict-object`,
# was enqueued, and exited 0.
#
# `audit_state` runs no command at all, so these cases run it with an EMPTY PATH: not `grep`
# broken, but no external command reachable from the function under test. A rewrite that reaches
# for one fails here whatever it reaches for.
empty_path="$tmp/empty-path"
mkdir -p "$empty_path"
state_of() {  # state_of MOVED BLOCKER...: the state those blockers put a pull request in
  local moved="$1"; shift
  blockers=("$@")
  state=""
  audit_state "$moved"
  printf '%s' "$state"
}
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "" manual:P1-outside-the-verdict-object)" MANUAL
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "" manual:a manual:b)" MANUAL
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "")" READY
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "" "")" READY
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "" no-review)" NOT-READY
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "" manual:a no-review)" NOT-READY
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of repairs manual:a)" NEEDS-ATTEST
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of merge-edits:abc1234 manual:a)" NEEDS-ATTEST
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of repairs no-review)" NOT-READY
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of ledger-only)" READY
# A hard blocker that merely CONTAINS the manual prefix is still hard: the test is the prefix, on
# the whole element, and `grep '^manual:'` over a printed list was matching line starts in text
# the elements themselves could have supplied.
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "" "open-P1:a-manual:thing")" NOT-READY
expect MUT-MANUAL-SCAN-FAILS-OPEN "$(PATH="$empty_path" state_of "" "verdict:x
manual:y")" NOT-READY

# --- the newest check run per name -------------------------------------------------------------
got="$(printf 'upstroke-ci\t100\tsuccess\nupstroke-ci\t250\tfailure\nupstroke-pr-policy\t120\tsuccess\nupstroke-ci\t90\tsuccess\n' | newest_per_name | tr ' ' '\n' | grep . | sort | tr '\n' ' ')"
expect MUT-CHECK-RUN-FIRST-SUCCESS "$got" "upstroke-ci=failure upstroke-pr-policy=success "
got="$(printf 'upstroke-ci\t300\tsuccess\nupstroke-ci\t200\tcancelled\n' | newest_per_name)"
expect MUT-CHECK-RUN-UNSTARTED "$got" "upstroke-ci=success "
got="$(printf 'upstroke-ci\t1000\tqueued\nupstroke-ci\t999\tsuccess\n' | newest_per_name)"
expect MUT-CHECK-RUN-UNSTARTED "$got" "upstroke-ci=queued "

# --- the ledger rows ---------------------------------------------------------------------------
cat > "$tmp/body.md" <<'EOF'
## Summary

Text with a | pipe.

## Review finding ledger

| ID | Severity | Reviewed SHA / location | Failure sequence | Provenance | Category | First bad / prior ID | Regression or documented guard | Disposition |
|---|---|---|---|---|---|---|---|---|
| A-DEFERRABLE | P3 | 4ad962f / src/x.rs:1 | a -> b | pre_existing | correctness | — | `guard` | deferred |
| B-FIXED | P2 | 4ad962f / src/y.rs:2 | a -> b | introduced_by_feature | liveness | — | `test` | fixed |

## Risk and rollback

| not | a | ledger | row |
EOF
ledger_rows() {  # ledger_rows FILE: "<status>|<id>:<disposition>;..."
  local out status=0
  out="$("$parser_python" scripts/pr-review-parse.py ledger "$1" 2>/dev/null)" || status=$?
  ((status == 0)) || { printf '%s|' "$status"; return 0; }
  printf '%s|%s' "$status" "$(printf '%s' "$out" | jq -j '[.rows[] | "\(.id):\(.disposition)"] | join(";")')"
}
expect MUT-LEDGER-HEADER-AS-ROW "$(ledger_rows "$tmp/body.md")" \
  '0|A-DEFERRABLE:deferred;B-FIXED:fixed'
# The payload the audit reads, whose second field is the row count: the same rule as the review's,
# so a ledger that stopped arriving is not a pull request with fewer rows in it.
expect MUT-PARSE-PAYLOAD-COUNT "$(parse_nul ledger "$tmp/body.md")" \
  '0|ledger|2|A-DEFERRABLE|deferred|B-FIXED|fixed|'
# A body the audit could not fetch is not a body with no ledger in it. It used to be `gh pr view
# ... | ledger_rows_from_body` captured into a variable, so a failed fetch ended the whole run
# through `set -e` -- not a decision about this pull request, just an exit -- and any rewrite of
# that line into a condition would have made it "no ledger rows", which is `no-row` for a finding
# that has a row. It is a blocker on this pull request now, and it is stated.
got="$(STUB_REVIEW_BODY="$tmp/truncating-review.md" STUB_COMMENTS_STATUS=0 STUB_BODY_STATUS=1 run_stub 999)"
contains MUT-LEDGER-LOOKUP-SUPPRESSED "$got" "ledger-lookup-failed"
contains MUT-LEDGER-LOOKUP-SUPPRESSED "$got" "NOT-READY"
# and the same run with the body readable does not report the failure, so the case above is about
# the fetch and not about the review.
got="$(STUB_REVIEW_BODY="$tmp/truncating-review.md" STUB_COMMENTS_STATUS=0 run_stub 999)"
[[ "$got" == *ledger-lookup-failed* ]] \
  && error "MUT-LEDGER-LOOKUP-SUPPRESSED: a readable body was reported unreadable"

if ((failed)); then
  echo "test-pr-ready-audit: FAILED" >&2
  exit 1
fi
echo "test-pr-ready-audit: ok"
