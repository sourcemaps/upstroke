#!/usr/bin/env bash
# validate-pr-commits.sh <target-sha> <head-sha>
#
# Refuse a pull request any of whose OWN commits credits Claude, from the repository in the
# current directory. The owner's direction of 2026-11-25, recorded at
# .github/master-rewrite-2026-11-25.md: master's history was rewritten to carry no such credit, and
# this is the check for the listed credit forms.
#
# The gate refuses, in a pull request's own commits, exactly:
#
# 1. An author or committer whose email is `noreply@anthropic.com` or whose name is exactly `Claude`.
# 2. A `Co-Authored-By` line, begun with the key or parsed by git as the trailer, whose mailbox
#    domain is `anthropic.com` or a subdomain, or whose display name holds `Claude` or `Anthropic`
#    as a whole word (`:283`, unchanged). A mailbox is what the first `<…>` holds, or a bracket-less
#    value ending in `@domain`, a trailing parenthesised comment aside. Its domain is what follows
#    its last `@`, if it has one; its local part is never parsed or searched. A value with no
#    mailbox is a name only where git parsed the trailer, and prose elsewhere.
# 3. A `Claude-Session:` line.
# 4. A whole line `Generated with Claude Code` or `Generated with [Claude Code]`, with an optional
#    link, final period and leading 🤖.
#
# Everything else passes. Listed limits: any other footer prefix or wording; `Claude` joined to
# other text by a hyphen, apostrophe or non-ASCII character, at another address; a mailbox-less
# line git does not parse; other keys; identities not listed. Listed false refusals: a person
# named Claude; any `anthropic.com` address; a sentence git parses as the trailer. A direct run
# ignores replace refs and fails closed on shallow history and grafts; beyond that the claim is
# for the workflow's checkout.
#
# The five rule IDs are claude-author, claude-committer, claude-co-author, claude-session and
# claude-generated. Comparisons are case-insensitive; identity names are trimmed. Keys allow
# leading whitespace and whitespace before the colon. The footer allows surrounding whitespace;
# only the bracketed label may have a Markdown link destination. The display name precedes the
# first `<…>`; text after it is a comment. Name words keep ASCII letters, digits, apostrophes,
# hyphens and UTF-8 bytes together. These matchers are unchanged from r72's starting head.
#
# CO-AUTHORS ARE JUDGED ONLY AFTER UNFOLDING, never from an incomplete physical-line prefix:
#
#   - every logical line of the message: a nonblank line that begins with whitespace and follows a line
#     whose key matches `^[[:space:]]*[A-Za-z0-9-]+[[:space:]]*:` is appended to that line, its
#     leading whitespace dropped and one space joining the two, as git's trailer folding does, so a
#     `Co-Authored-By:` whose identity sits on an indented continuation line is judged whole (all
#     message rules). Empty AND whitespace-only lines end a logical value, as Git's paragraph
#     separators do. This also retains the owner's standalone credits outside Git's final trailer
#     block: an explicit co-author key starts its own value even when indented after another key.
#     Session keys and generated-with footers are additionally recognized as written;
#   - every line `git interpret-trailers --parse --no-divider` prints for the message, which is what
#     git itself takes for the trailers (the co-author and session rules). `--no-divider` because
#     that is how git reads a commit's trailers: a `---` in a commit message ends nothing. The
#     message is the object's own message part, written to a file and parsed from it, never
#     through a pipe whose status is lost.
#
# Each rule is reported at most ONCE per commit, with the first line that broke it, so a credit
# found by two of those readings, or written twice, is one refusal line and not two.
#
# THE COMMITS JUDGED ARE THE PULL REQUEST'S OWN: every commit reachable from the head and from no
# merge base of the two ends. That set is exactly the commits the head carries that the target
# does not -- every common ancestor of the two is reachable from one of the best ones `git
# merge-base --all` names -- and EVERY merge base is excluded, not the first: with criss-crossed
# histories, excluding one would put the other's ancestry, which is the target's own history, in
# front of this check. A commit reachable from the target alone is never judged. That is not a
# courtesy: commits on master from before the rewrite carry `Claude-Session:` lines, and a rule
# over them would turn every pull request in the repository red and leave nothing a pull request
# could do about it. A pull request that merges master in brings those commits along, and they are
# still not its own: they are reachable from the target, so they are still not judged.
#
# GRAFTS, SHALLOW OR UNAVAILABLE ANCESTRY, AN END THAT WILL NOT RESOLVE, OR TWO ENDS WITH NO MERGE BASE
# FAILS CLOSED at exit 2, never with
# an empty set. An empty set is a pass, and a pass that nothing was checked for is the direction
# that turns a refusal into an acceptance. Every git command that fails after that is exit 2 too.
#
# THE RECORD IS THE RAW COMMIT OBJECT, `git cat-file commit`, AND NOT A `git log` RENDERING. A
# placeholder such as `%an` is a rendering, which config the runner or the repository brings --
# a .mailmap, an output encoding -- can reach, and a check a pull request could steer by adding a
# .mailmap to its own tree is not a check. The object is what the commit IS.
#
# A MESSAGE CANNOT FORGE A RECORD BOUNDARY. The commit list is `git rev-list`'s, one object name
# per line, and every line is required to be a full hexadecimal object name before it is used, so
# nothing a commit says can add, drop or rename an entry. Each commit is then read alone, from its
# own object: the headers end at the first empty line, and git writes the message after that line,
# so no message line can be taken for an `author` or `committer` header, and no text in one commit
# can be read as part of another. Every `author` and `committer` header is judged, not the first,
# so a hand-built object carrying two cannot hide the second behind the first.
#
# EVERY LISTING IS WRITTEN TO A FILE AND READ TO ITS END: the merge bases, the commit list, each
# commit object and each commit's parsed trailers. git's status is taken from the command that
# wrote the file, where it is a status; through a pipe or `< <(...)` it belongs to nobody, and a
# failed rev-list would read as a pull request with no commits. And a read that fails mid-file
# looks to `read` exactly like the end of it, so the number of lines written is counted first and
# the loop must reach it: a short read is exit 2, never fewer lines judged. Object-name lists also
# account for every byte and require the final newline before any commit is judged. A short read of the
# merge bases is the dangerous one -- an exclusion list left empty judges the target's own history
# -- so each of its lines must also be a full object name of the repository's own width. These are the
# rules .github/scripts/changed-in-range.sh and test-docs-consistency.sh state for their own
# listings.
#
# NOTHING THE SCRIPT PRINTS CARRIES AN EMAIL ADDRESS: refusals and the pass line pass through one
# redaction. Failures use concise diagnostics; raw command errors and input paths are never printed.
# A refusal is one line on stderr per
# rule a commit breaks:
#
#   claude-credit: <full-sha> <rule-id>: <the author or committer's name, or the message line>
#
# An identity is shown by its name alone, and in any printed text every token holding an `@` is
# replaced by `[email redacted]`, so a `Co-Authored-By:` line naming somebody's address, or an
# unresolvable end shaped like one, names nobody's in the log. Control characters are shown as
# `?`, so a line cannot carry a carriage return or an escape sequence into the runner's log. Every
# other message begins `commit-credit-policy:`, and a pass is one line on stdout:
#
#   commit-credit-policy: <n> commit(s) checked, none credits Claude

set -Eeuo pipefail
export PATH="/usr/bin:/bin:$PATH"
# Byte-wise classes and ASCII case folding, whatever locale the runner has. Name boundaries
# explicitly retain high bytes, so UTF-8 names do not depend on installed locales.
export LC_ALL=C
# A replace ref swaps one object for another under the same name. The object judged is the object
# the name was written for.
export GIT_NO_REPLACE_OBJECTS=1
# Missing objects must fail locally, not trigger a promisor remote fetch.
export GIT_NO_LAZY_FETCH=1
shopt -s nocasematch

# Keep the diagnostic channel separate from command and shell errors (including redirection
# failures, whose text can contain private paths). Only our deliberate output uses descriptor 3.
exec 3>&2 2>/dev/null

# shown <text>: the text as this script may print it -- control characters as `?`, and every
# token holding an `@` replaced, so no email address reaches the log.
shown() {
  local text="${1//[[:cntrl:]]/?}"
  printf '%s\n' "$text" | sed -E 's/[^[:space:]<>]*@[^[:space:]<>]*/[email redacted]/g'
}

say() {  # say <text>: one `commit-credit-policy:` line, redacted
  printf 'commit-credit-policy: %s\n' "$(shown "$1")"
}

fail() {  # fail <text>: the closed failure, redacted, at exit 2
  say "$1" >&3
  exit 2
}

trap 'fail "could not complete validation"' ERR
work=""
# Even redacted git errors can contain private paths or raw identity names.
fail_git() { fail "$1"; }

target="${1:-}"
head="${2:-}"

if [[ -z "$target" || -z "$head" ]]; then
  say "usage: validate-pr-commits.sh <target-sha> <head-sha>" >&3
  exit 2
fi

# Do not echo the supplied ends: an invalid ref may itself be a private path or identity.
target_sha="$(git rev-parse --verify --quiet "$target^{commit}" 2> /dev/null)" \
  || fail "target commit is not in this checkout"
head_sha="$(git rev-parse --verify --quiet "$head^{commit}" 2> /dev/null)" \
  || fail "head commit is not in this checkout"

work="$(mktemp -d)" || fail "could not create temporary workspace"
[[ -n "$work" && -d "$work" ]] || fail "could not create temporary workspace"
trap 'rm -rf -- "$work"' EXIT

# lines_in <file>: the number of newline-terminated lines in it, measured before a loop reads it.
lines_in() {
  local n
  n="$(wc -l < "$1")" || return 1
  echo $(( n ))
}

object_format="$(git rev-parse --show-object-format=storage)" \
  || fail "could not determine the repository object format"
case "$object_format" in
  sha1) object_name_re='^[0-9a-f]{40}$' ;;
  sha256) object_name_re='^[0-9a-f]{64}$' ;;
  *) fail "could not determine the repository object format" ;;
esac
[[ "$target_sha" =~ $object_name_re && "$head_sha" =~ $object_name_re ]] \
  || fail "could not resolve complete commit IDs"

# Grafts change ancestry even with replace refs disabled. Resolve the common git path (also
# in a linked worktree), respecting Git's explicit graft-file override. Comments are inert.
grafts="${GIT_GRAFT_FILE-}"
if [[ ! ${GIT_GRAFT_FILE+set} ]]; then
  grafts="$(git rev-parse --git-path info/grafts)" \
    || fail "could not determine history completeness"
fi
if [[ -e "$grafts" ]]; then
  [[ -f "$grafts" && -r "$grafts" ]] || fail "complete local history is unavailable"
  if grep -q '^[[:space:]]*[^#[:space:]]' -- "$grafts"; then
    fail "complete local history is unavailable"
  else
    [[ "$?" == 1 ]] || fail "complete local history is unavailable"
  fi
fi

# A merge base can exist in a shallow checkout even though a merged side branch's offending
# ancestor is hidden. Walk BOTH ends without exclusions, so missing ancestors below the merge
# bases cannot be mistaken for a complete (possibly empty) PR range either.
shallow="$(git rev-parse --is-shallow-repository)" \
  || fail "could not determine history completeness"
[[ "$shallow" == false ]] || fail "complete local history is unavailable"
git rev-list "$target_sha" "$head_sha" -- > /dev/null \
  || fail "complete local history is unavailable"

# read_object_names <file> <label>: populate names only from complete newline-terminated records.
# Counting bytes as well as records detects read failures and bytes Bash read silently drops
# (notably NUL). EOF is acceptable only after every measured byte has been accounted for.
read_object_names() {
  local file="$1" label="$2" bytes consumed=0 record status
  names=()
  bytes="$(wc -c < "$file")" || fail "could not measure the $label list"
  [[ "$bytes" =~ ^[[:space:]]*[0-9]+[[:space:]]*$ ]] \
    || fail "could not measure the $label list"
  while :; do
    record=""
    status=0
    IFS= read -r record || status=$?
    if (( status != 0 )); then
      [[ -z "$record" ]] || fail "the $label list holds an incomplete object-name record"
      (( status == 1 )) || fail "could not read the $label list"
      break
    fi
    [[ "$record" =~ $object_name_re ]] \
      || fail "the $label list holds a line that is not an object name"
    consumed=$(( consumed + ${#record} + 1 ))
    names+=("$record")
  done < "$file"
  (( consumed == bytes )) || fail "could not read the complete $label list"
}

# No common ancestor is not an empty range: it means the two ends are unrelated histories, and the
# head's whole history is not this pull request's to answer for.
git merge-base --all "$target_sha" "$head_sha" > "$work/merge-bases" 2> "$work/git-err" \
  || fail_git "no merge base between $target_sha and $head_sha"
[[ -s "$work/merge-bases" ]] \
  || fail "no merge base between $target_sha and $head_sha"

names=()
read_object_names "$work/merge-bases" merge-base
bases=("${names[@]}")

# No pathspec, so no history simplification: a commit reached only through a merge of a side
# branch is listed like any other, and so is the merge.
git rev-list "$head_sha" --not "${bases[@]}" > "$work/commits" 2> "$work/git-err" \
  || fail_git "could not list the commits between the merge base and $head_sha"
read_object_names "$work/commits" commit

refusals=0
hits=" "
refuse() {  # refuse <sha> <rule-id> <text>: at most once per rule per commit
  [[ "$hits" == *" $2 "* ]] && return 0
  hits="$hits$2 "
  printf 'claude-credit: %s %s: %s\n' "$1" "$2" "$(shown "$3")" >&3
  refusals=$(( refusals + 1 ))
}

trim() {  # trim <text>: the text without its leading and trailing whitespace
  local text="$1"
  text="${text#"${text%%[![:space:]]*}"}"
  printf '%s' "${text%"${text##*[![:space:]]}"}"
}

# is_claude <name> <email>: the identity test both identity rules share. The name is trimmed of
# surrounding whitespace before it is compared; the email is compared as written.
is_claude() {
  [[ "$2" == "noreply@anthropic.com" || "$(trim "$1")" == "Claude" ]]
}

# An identity header is `<role> <name> <<email>> <date> <zone>`. The pattern is held in a variable
# because `\<` written inline is a word boundary to some regex libraries and not a `<`.
identity_re='^(author|committer) ([^<]*)<([^>]*)>'

# The trailer keys, and the key any line must have for an indented line after it to fold into it.
fold_key_re='^[[:space:]]*[A-Za-z0-9-]+[[:space:]]*:'
co_author_re='^[[:space:]]*co-authored-by[[:space:]]*:(.*)$'
session_re='^[[:space:]]*claude-session[[:space:]]*:'
# A value's `<address>`, and the name before it. Held in variables for the same reason as above.
address_re='^([^<]*)<([^>]*)>(.*)$'
bare_mailbox_re='^([^<>]*@[^[:space:]<>@()]+)([[:space:]]*\(.*\))?$'
# Keep every UTF-8 byte and compound-name apostrophe/hyphen within a name word. This avoids
# depending on a UTF-8 locale that need not exist on all supported runners.
word_re=$'(^|[^A-Za-z0-9\'\200-\377-])(claude|anthropic)($|[^A-Za-z0-9\'\200-\377-])'
# A whole footer, not an arbitrary punctuation prefix or a sentence beginning with the label.
footer_re='^[[:space:]]*(🤖[[:space:]]*)?generated with (\[claude code\](\([^()[:space:]]+\))?|claude code)\.?[[:space:]]*$'

# names_claude <co-author value> <parsed>: only git-parsed trailers treat mailbox-less values
# as names. Mailbox local parts are opaque, including quotes, spaces and additional @ signs.
names_claude() {
  local value name address="" domain
  value="$(trim "$1")"
  if [[ "$value" =~ $address_re ]]; then
    name="${BASH_REMATCH[1]}"
    address="$(trim "${BASH_REMATCH[2]}")"
  elif [[ "$value" =~ $bare_mailbox_re ]]; then
    address="${BASH_REMATCH[1]}"
    name=""
  else
    [[ "$2" == parsed ]] || return 1
    name="$value"
  fi
  if [[ "$address" == *@* ]]; then
    domain="${address##*@}"
    if [[ "$domain" == "anthropic.com" || "$domain" == *".anthropic.com" ]]; then
      return 0
    fi
  fi
  [[ "$name" =~ $word_re ]]
}

# judge_trailer <sha> <line> <parsed|unparsed>: the two rules that read trailers.
judge_trailer() {
  if [[ "$2" =~ $co_author_re ]] && names_claude "${BASH_REMATCH[1]}" "$3"; then
    refuse "$1" claude-co-author "$2"
  fi
  if [[ "$2" =~ $session_re ]]; then
    refuse "$1" claude-session "$2"
  fi
}

# judge_line <sha> <line>: all three message-line rules.
judge_line() {
  judge_trailer "$1" "$2" unparsed
  if [[ "$2" =~ $footer_re ]]; then
    refuse "$1" claude-generated "$2"
  fi
}

# judge <sha>: every refusal the commit earns, one line per rule.
judge() {
  local sha="$1" object="$work/object" message_file="$work/message" trailers="$work/trailers"
  local line in_headers=1 authors=0 committers=0 read_lines=0 written parsed read_trailers=0
  local role name email rule last
  local message=() logical=()
  hits=" "
  git cat-file commit "$sha" > "$object" 2> "$work/git-err" \
    || fail_git "could not read commit $sha"
  # A final newline, so the last message line is a whole line and the count below counts it. An
  # empty line at the end of a message is nothing any rule looks at.
  printf '\n' >> "$object"
  written="$(lines_in "$object")" \
    || fail "could not measure commit $sha"
  while IFS= read -r line; do
    read_lines=$(( read_lines + 1 ))
    if (( in_headers )); then
      if [[ -z "$line" ]]; then
        in_headers=0
        continue
      fi
      # A continuation line of a multi-line header (a signature, a merged tag) begins with a space
      # and is never an identity of this commit.
      case "$line" in
        'author '* | 'committer '*) ;;
        *) continue ;;
      esac
      [[ "$line" =~ $identity_re ]] \
        || fail "commit $sha carries an identity header git could not have written"
      role="${BASH_REMATCH[1]}"
      name="${BASH_REMATCH[2]}"
      email="${BASH_REMATCH[3]}"
      rule="claude-author"
      if [[ "$role" == committer ]]; then
        rule="claude-committer"
        committers=$(( committers + 1 ))
      else
        authors=$(( authors + 1 ))
      fi
      if is_claude "$name" "$email"; then
        name="$(trim "$name")"
        refuse "$sha" "$rule" "${name:-<empty name>}"
      fi
      continue
    fi
    message+=("$line")
    # These standalone rules do not interpret a co-author identity. Never judge a physical
    # co-author prefix: its continuation can change which bytes are name, mailbox or comment.
    if [[ "$line" =~ $session_re ]]; then
      refuse "$sha" claude-session "$line"
    fi
    if [[ "$line" =~ $footer_re ]]; then
      refuse "$sha" claude-generated "$line"
    fi
  done < "$object"
  (( read_lines == written )) \
    || fail "read $read_lines of the $written lines of commit $sha"
  # An author and a committer, at the least: a commit without both was not read as a commit.
  (( authors >= 1 && committers >= 1 )) \
    || fail "commit $sha carries no author or no committer"

  # The message unfolded: only a nonblank indented line continues a keyed line. A paragraph
  # separator ends the value even when it contains spaces or tabs. An explicit co-author key
  # starts a new standalone credit, including after another keyed line (the owner's rule).
  for line in ${message[@]+"${message[@]}"}; do
    last=$(( ${#logical[@]} - 1 ))
    if (( last >= 0 )) && [[ "$line" =~ ^[[:space:]] && "$line" =~ [^[:space:]] &&
        ! "$line" =~ $co_author_re && "${logical[$last]}" =~ $fold_key_re ]]; then
      logical[$last]="$(trim "${logical[$last]}") $(trim "$line")"
    else
      logical+=("$line")
    fi
  done
  for line in ${logical[@]+"${logical[@]}"}; do
    judge_line "$sha" "$line"
  done

  # And what git takes for the trailers, parsed from the message part alone.
  printf '%s\n' ${message[@]+"${message[@]}"} > "$message_file" \
    || fail "could not write the message of commit $sha"
  git interpret-trailers --parse --no-divider "$message_file" > "$trailers" 2> "$work/git-err" \
    || fail_git "could not parse the trailers of commit $sha"
  parsed="$(lines_in "$trailers")" \
    || fail "could not measure the trailers of commit $sha"
  while IFS= read -r line; do
    read_trailers=$(( read_trailers + 1 ))
    judge_trailer "$sha" "$line" parsed
  done < "$trailers"
  (( read_trailers == parsed )) \
    || fail "read $read_trailers of the $parsed trailer lines of commit $sha"
}

checked=0
for sha in ${names[@]+"${names[@]}"}; do
  judge "$sha"
  checked=$(( checked + 1 ))
done

if (( refusals > 0 )); then
  exit 1
fi
say "$checked commit(s) checked, none credits Claude"
