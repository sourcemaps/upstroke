#!/usr/bin/env bash
# validate-pr-commits.sh <target-sha> <head-sha>
#
# ASCII case-insensitively, the gate refuses a pull request's own commit when:
# - git reports its author or committer email as `noreply@anthropic.com` or ending `+claude[bot]@users.noreply.github.com`, or the trimmed name as `Claude` or `claude[bot]` (r72's question (c): yes, this one identity); or
# - its message contains `noreply@anthropic.com`, `Co-Authored-By: Claude`, `Claude-Session:`, `Generated with Claude Code` or `Generated with [Claude Code]`.
#
# Nothing is parsed. Every other spelling passes: a listed limit. A message quoting a listed string is refused: a listed false refusal. The claim is for the workflow's checkout.
#
# Beside the Contract (ruling r116): a commit that declares an encoding (git's `%e` is not empty) is not judged, and the gate stops at exit 2. git may decode such an object before it reports its fields, so what it reports need not be what the object holds.
#
# It checks the repository in the current directory. The owner's direction of 2026-11-25,
# recorded at .github/master-rewrite-2026-11-25.md: master's history was rewritten to carry no
# such credit, and this is the check for the listed strings.
#
# EVERY COMPARISON IS OF FIXED STRINGS, the ASCII letters folded and every other byte compared as
# it is. A listed string is never a glob, a bracket expression or a regular expression: `[bot]`
# and `[Claude Code]` are those characters and not a choice of one, so neither `claudeb` nor
# `Generated with a` is refused.
#
# WHAT IS COMPARED IS WHAT GIT'S OWN FORMATTER REPORTS for each commit the range lists, and
# nothing else in the commit:
#
#   - its identities, `%an %ae %cn %ce`: the raw fields, never the mailmap forms
#     (`%aN %aE %cN %cE`), so a .mailmap a pull request adds to its own tree renames nobody. An
#     identity is refused when its email equals `noreply@anthropic.com` or ends with
#     `+claude[bot]@users.noreply.github.com`, or its name, with the surrounding whitespace
#     trimmed, equals `Claude` or `claude[bot]`: the author's fields give rule claude-author and
#     the committer's claude-committer. A header line git does not report as either, such as a
#     hand-built `Author` line, is nobody's identity;
#   - its message, `%B`. It is refused when it holds `noreply@anthropic.com` or
#     `Co-Authored-By: Claude` (rule claude-co-author), `Claude-Session:` (rule claude-session), or
#     `Generated with Claude Code` or `Generated with [Claude Code]` (rule claude-generated). No
#     listed string holds a newline, so a message holds one exactly when one of its lines does.
#
# Every per-commit read is `git log --no-walk --no-show-signature --encoding=UTF-8`, so
# `log.showSignature`, `i18n.logOutputEncoding` and `i18n.commitEncoding` change no verdict.
#
# Those are the five rule IDs. Each is reported at most ONCE per commit: an identity rule with the
# trimmed name, and a message rule with the first message line that holds one of its strings, so a
# commit holding a rule's strings twice, or both of them, is one refusal line and not two.
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
# GRAFTS, SHALLOW OR UNAVAILABLE ANCESTRY, AN END THAT WILL NOT RESOLVE, OR TWO ENDS WITH NO MERGE
# BASE FAILS CLOSED at exit 2, never with an empty set. An empty set is a pass, and a pass that
# nothing was checked for is the direction that turns a refusal into an acceptance. GIT_GRAFT_FILE
# set at all, even to an empty value, is exit 2 before any git command runs, so no file it names is
# resolved or read; with it unset, a repository whose own info/grafts holds a graft is exit 2.
# Replace refs are ignored. Every git command that fails after that is exit 2 too.
#
# A MESSAGE CANNOT FORGE A RECORD BOUNDARY. The commit list is `git rev-list`'s, one object name
# per line, and every line is required to be a full hexadecimal object name before it is used, so
# nothing a commit says can add, drop or rename an entry. Each commit is then read alone, by its
# own name: its `%e` into one file, which must hold one empty line, its four identity fields
# into a second, a field to a line -- git keeps each identity on one header line, so no field can
# hold a newline -- and its message into a third. No message line can be taken for an identity,
# and no text in one commit can be read as part of another.
#
# EVERY LISTING IS WRITTEN TO A FILE AND READ TO ITS END: the merge bases, the commit list, and
# each commit's `%e`, identities and message. git's status is taken from the command that wrote the
# file, where it is a status; through a pipe or `< <(...)` it belongs to nobody, and a failed
# rev-list would read as a pull request with no commits. And a read that fails mid-file looks to
# `read` exactly like the end of it, so the bytes written are measured first and the read must
# account for every one of them, the final newline included: a short read is exit 2, never fewer
# lines judged, and a commit's identities must be exactly its four fields. A short read of the
# merge bases is the dangerous one -- an exclusion list left empty judges the target's own history
# -- so each of its lines must also be a full object name of the repository's own width. These are
# the rules .github/scripts/changed-in-range.sh and test-docs-consistency.sh state for their own
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
# Byte-wise classes and ASCII case folding, whatever locale the runner has: under nocasematch
# below, a comparison folds the ASCII letters and compares every other byte as it is.
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

# GIT_GRAFT_FILE would name a graft file for every git command below, and git resolves a relative
# name from the top of the work tree, not from here. Set at all, even to an empty value, it is
# refused before any git command runs.
[[ ! ${GIT_GRAFT_FILE+set} ]] || fail "complete local history is unavailable"

# Do not echo the supplied ends: an invalid ref may itself be a private path or identity.
target_sha="$(git rev-parse --verify --quiet "$target^{commit}" 2> /dev/null)" \
  || fail "target commit is not in this checkout"
head_sha="$(git rev-parse --verify --quiet "$head^{commit}" 2> /dev/null)" \
  || fail "head commit is not in this checkout"

work="$(mktemp -d)" || fail "could not create temporary workspace"
[[ -n "$work" && -d "$work" ]] || fail "could not create temporary workspace"
trap 'rm -rf -- "$work"' EXIT

object_format="$(git rev-parse --show-object-format=storage)" \
  || fail "could not determine the repository object format"
case "$object_format" in
  sha1) object_name_re='^[0-9a-f]{40}$' ;;
  sha256) object_name_re='^[0-9a-f]{64}$' ;;
  *) fail "could not determine the repository object format" ;;
esac
[[ "$target_sha" =~ $object_name_re && "$head_sha" =~ $object_name_re ]] \
  || fail "could not resolve complete commit IDs"

# Grafts change ancestry even with replace refs disabled. GIT_GRAFT_FILE was refused above, so
# resolve the common git path (also in a linked worktree). Comments are inert.
grafts="$(git rev-parse --git-path info/grafts)" \
  || fail "could not determine history completeness"
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

# read_lines <file> <label>: lines=() gets every line of the file, read as read_object_names reads
# a list: the bytes are measured first, and the end of the file is accepted only once every one of
# them is accounted for, the last line's newline included.
read_lines() {
  local file="$1" label="$2" bytes consumed=0 line status
  lines=()
  bytes="$(wc -c < "$file")" || fail "could not measure $label"
  [[ "$bytes" =~ ^[[:space:]]*[0-9]+[[:space:]]*$ ]] || fail "could not measure $label"
  while :; do
    line=""
    status=0
    IFS= read -r line || status=$?
    if (( status != 0 )); then
      [[ -z "$line" ]] || fail "$label ends part-way through a line"
      (( status == 1 )) || fail "could not read $label"
      break
    fi
    consumed=$(( consumed + ${#line} + 1 ))
    lines+=("$line")
  done < "$file"
  (( consumed == bytes )) || fail "could not read all of $label"
}

# claude_identity <trimmed name> <email>: the rule both identity fields share. Every listed string
# is quoted, so it is compared as a fixed string, `[bot]` as those five characters, and nocasematch
# folds the ASCII letters.
claude_identity() {
  [[ "$2" == "noreply@anthropic.com" || "$2" == *"+claude[bot]@users.noreply.github.com" ||
     "$1" == "Claude" || "$1" == "claude[bot]" ]]
}

# commit_log <format> <sha> --: git log over one commit, as every read in judge runs it, so no
# signature check writes a line ahead of the fields and git reports UTF-8 whatever the
# repository's i18n settings say.
commit_log() {
  git log --no-walk --no-show-signature --encoding=UTF-8 "$@"
}

# judge <sha>: every refusal the commit earns, one line per rule.
judge() {
  local sha="$1" author committer line
  hits=" "
  # The encoding the commit declares, before any other field. git decodes a commit that declares
  # one before it reports a field, so such a commit is not judged: the read must be exactly one
  # line, and that line empty.
  commit_log --format=%e "$sha" -- > "$work/encoding" 2> "$work/git-err" \
    || fail_git "could not read the encoding of commit $sha"
  read_lines "$work/encoding" "the encoding of commit $sha"
  if (( ${#lines[@]} != 1 )) || [[ -n "${lines[0]}" ]]; then
    fail "commit $sha declares an encoding"
  fi
  # The raw fields, a field to a line, and never the mailmap forms.
  commit_log --format='%an%n%ae%n%cn%n%ce' "$sha" -- > "$work/identities" 2> "$work/git-err" \
    || fail_git "could not read the identities of commit $sha"
  read_lines "$work/identities" "the identities of commit $sha"
  (( ${#lines[@]} == 4 )) || fail "could not read the identities of commit $sha"
  author="$(trim "${lines[0]}")"
  committer="$(trim "${lines[2]}")"
  if claude_identity "$author" "${lines[1]}"; then
    refuse "$sha" claude-author "${author:-<empty name>}"
  fi
  if claude_identity "$committer" "${lines[3]}"; then
    refuse "$sha" claude-committer "${committer:-<empty name>}"
  fi

  # The message. No listed string holds a newline, so the message holds one exactly when one of
  # its lines does, and refuse() shows the first line that holds the rule's string.
  commit_log --format=%B "$sha" -- > "$work/message" 2> "$work/git-err" \
    || fail_git "could not read the message of commit $sha"
  read_lines "$work/message" "the message of commit $sha"
  for line in ${lines[@]+"${lines[@]}"}; do
    if [[ "$line" == *"noreply@anthropic.com"* || "$line" == *"Co-Authored-By: Claude"* ]]; then
      refuse "$sha" claude-co-author "$line"
    fi
    if [[ "$line" == *"Claude-Session:"* ]]; then
      refuse "$sha" claude-session "$line"
    fi
    if [[ "$line" == *"Generated with Claude Code"* ||
          "$line" == *"Generated with [Claude Code]"* ]]; then
      refuse "$sha" claude-generated "$line"
    fi
  done
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
