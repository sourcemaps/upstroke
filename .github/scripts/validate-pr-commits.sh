#!/usr/bin/env bash
# validate-pr-commits.sh <target-sha> <head-sha>
#
# Refuse a pull request any of whose OWN commits credits Claude, from the repository in the
# current directory. The owner's direction of 2026-11-25, recorded at
# .github/master-rewrite-2026-11-25.md: master's history was rewritten to carry no such credit, and
# this is the check that keeps a pull request from bringing it back. A commit is refused when any
# of five rules holds, each named by its id in the refusal:
#
#   claude-author      the author's email is noreply@anthropic.com, or the author's name, trimmed,
#                      is exactly `Claude` -- both case-insensitive;
#   claude-committer   the same two tests on the committer;
#   claude-co-author   a message line that, after leading whitespace, begins `Co-Authored-By:` and
#                      names `claude` or `anthropic` anywhere after it;
#   claude-session     a message line that, after leading whitespace, begins `Claude-Session:`;
#   claude-generated   a message line holding `Generated with [Claude Code]` or
#                      `Generated with Claude Code`.
#
# Every comparison is case-insensitive, because none of these is spelled one way by every tool
# that writes it, and git preserves whatever case it was given.
#
# EACH RULE IS THE SHAPE IT IS SO THAT PROSE PASSES. Upstroke drives Claude Code as one of its
# agents, so a subject such as `fix(agent): the Claude Code adapter reads its version` is a
# legitimate commit and a rule that refused the words `Claude Code` would refuse it. So:
#
#   A NAME MATCHES ONLY WHOLE. `Claude` and nothing else, after trimming, is the name Claude Code
#   commits under; a contributor whose name merely begins or contains it is somebody else. The
#   email is the address Claude Code writes, matched whole for the same reason.
#
#   A TRAILER MATCHES ONLY AT THE START OF ITS LINE. `Co-Authored-By:` and `Claude-Session:` are
#   trailers, and a trailer is a line that begins with its key. A sentence that mentions
#   `Claude-Session` in passing -- this paragraph, a commit message about the rewrite -- is not one.
#   Leading whitespace is allowed before the key because an indented trailer still reads as a
#   credit to anyone who looks, and the rule exists for what a reader sees. The co-author rule
#   then needs `claude` or `anthropic` AFTER the key, so a co-author who is a person passes and
#   any Anthropic address, or any Claude model's name, does not.
#
#   THE GENERATED-WITH FOOTER MATCHES ANYWHERE IN ITS LINE, because it is written behind an emoji
#   (`🤖 Generated with [Claude Code](...)`) and has no key to anchor on. Both phrasings are named
#   whole, so `generated with` in prose about something else passes.
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
# AN END THAT WILL NOT RESOLVE, OR TWO ENDS WITH NO MERGE BASE, FAILS CLOSED at exit 2, never with
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
# EVERY LISTING IS WRITTEN TO A FILE AND READ TO ITS END. git's status is taken from the command
# that wrote the file, where it is a status; through a pipe or `< <(...)` it belongs to nobody, and
# a failed rev-list would read as a pull request with no commits. And a read that fails mid-file
# looks to `read` exactly like the end of it, so the number of lines written is counted first and
# the loop must reach it: a short read is exit 2, never fewer lines judged. These are the rules
# .github/scripts/changed-in-range.sh and test-docs-consistency.sh state for their own listings.
#
# THE OUTPUT NEVER CARRIES AN EMAIL ADDRESS. A refusal is one line on stderr per rule hit:
#
#   claude-credit: <full-sha> <rule-id>: <the author or committer's name, or the message line>
#
# An identity is shown by its name alone, and in a message line every token holding an `@` is
# replaced by `[email redacted]`, so a `Co-Authored-By:` line naming somebody's address names
# nobody's in the log. Control characters are shown as `?`, so a line cannot carry a carriage
# return or an escape sequence into the runner's log. A pass is one line on stdout:
#
#   commit-credit-policy: <n> commit(s) checked, none credits Claude

set -euo pipefail
export PATH="/usr/bin:/bin:$PATH"
# Byte-wise classes and ASCII case folding, whatever locale the runner has: every rule here is
# spelled in ASCII, and a multibyte locale would only add ways for a class to mean something else.
export LC_ALL=C
# A replace ref swaps one object for another under the same name. The object judged is the object
# the name was written for.
export GIT_NO_REPLACE_OBJECTS=1
shopt -s nocasematch

target="${1:-}"
head="${2:-}"

if [[ -z "$target" || -z "$head" ]]; then
  echo "usage: validate-pr-commits.sh <target-sha> <head-sha>" >&2
  exit 2
fi

target_sha="$(git rev-parse --verify --quiet "$target^{commit}")" \
  || { echo "commit-credit-policy: target commit $target is not in this checkout" >&2; exit 2; }
head_sha="$(git rev-parse --verify --quiet "$head^{commit}")" \
  || { echo "commit-credit-policy: head commit $head is not in this checkout" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT

# No common ancestor is not an empty range: it means the two ends are unrelated histories, and the
# head's whole history is not this pull request's to answer for.
git merge-base --all "$target_sha" "$head_sha" > "$work/merge-bases" \
  || { echo "commit-credit-policy: no merge base between $target_sha and $head_sha" >&2; exit 2; }
[[ -s "$work/merge-bases" ]] \
  || { echo "commit-credit-policy: no merge base between $target_sha and $head_sha" >&2; exit 2; }

bases=()
while IFS= read -r merge_base; do
  bases+=("$merge_base")
done < "$work/merge-bases"

# No pathspec, so no history simplification: a commit reached only through a merge of a side
# branch is listed like any other, and so is the merge.
git rev-list "$head_sha" --not "${bases[@]}" > "$work/commits" \
  || { echo "commit-credit-policy: could not list the commits between the merge base and $head_sha" >&2; exit 2; }

# lines_in <file>: the number of newline-terminated lines in it, measured before a loop reads it.
lines_in() {
  local n
  n="$(wc -l < "$1")" || return 1
  echo $(( n ))
}

# shown <text>: the text as a refusal line may print it -- control characters as `?`, and every
# token holding an `@` replaced, so no email address reaches the log.
shown() {
  local text="${1//[[:cntrl:]]/?}"
  printf '%s\n' "$text" | sed -E 's/[^[:space:]<>]*@[^[:space:]<>]*/[email redacted]/g'
}

refusals=0
refuse() {  # refuse <sha> <rule-id> <text>
  printf 'claude-credit: %s %s: %s\n' "$1" "$2" "$(shown "$3")" >&2
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

# judge <sha>: every refusal the commit earns, one line each.
judge() {
  local sha="$1" object="$work/object" line in_headers=1 authors=0 committers=0 read_lines=0 written
  local role name email rule
  git cat-file commit "$sha" > "$object" \
    || { echo "commit-credit-policy: could not read commit $sha" >&2; exit 2; }
  # A final newline, so the last message line is a whole line and the count below counts it. An
  # empty line at the end of a message is nothing any rule looks at.
  printf '\n' >> "$object"
  written="$(lines_in "$object")" \
    || { echo "commit-credit-policy: could not measure commit $sha" >&2; exit 2; }
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
        || { echo "commit-credit-policy: commit $sha carries an identity header git could not have written" >&2; exit 2; }
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
    if [[ "$line" =~ ^[[:space:]]*Co-Authored-By:(.*)$ ]] \
       && [[ "${BASH_REMATCH[1]}" == *claude* || "${BASH_REMATCH[1]}" == *anthropic* ]]; then
      refuse "$sha" claude-co-author "$line"
    fi
    if [[ "$line" =~ ^[[:space:]]*Claude-Session: ]]; then
      refuse "$sha" claude-session "$line"
    fi
    if [[ "$line" == *"Generated with [Claude Code]"* || "$line" == *"Generated with Claude Code"* ]]; then
      refuse "$sha" claude-generated "$line"
    fi
  done < "$object"
  (( read_lines == written )) \
    || { echo "commit-credit-policy: read $read_lines of the $written lines of commit $sha" >&2; exit 2; }
  # An author and a committer, at the least: a commit without both was not read as a commit.
  (( authors >= 1 && committers >= 1 )) \
    || { echo "commit-credit-policy: commit $sha carries no author or no committer" >&2; exit 2; }
}

listed="$(lines_in "$work/commits")" \
  || { echo "commit-credit-policy: could not measure the commit list" >&2; exit 2; }
checked=0
while IFS= read -r sha; do
  [[ "$sha" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]] \
    || { echo "commit-credit-policy: the commit list holds a line that is not an object name" >&2; exit 2; }
  judge "$sha"
  checked=$(( checked + 1 ))
done < "$work/commits"
(( checked == listed )) \
  || { echo "commit-credit-policy: read $checked of the $listed commits listed" >&2; exit 2; }

if (( refusals > 0 )); then
  exit 1
fi
echo "commit-credit-policy: $checked commit(s) checked, none credits Claude"
