#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/bin:/bin:$PATH"

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
validator="$script_dir/validate-pr-ledger-evidence.sh"
scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT

git -C "$scratch" init -q
git -C "$scratch" config user.name 'ledger fixture'
git -C "$scratch" config user.email 'ledger-fixture@upstroke.local'
mkdir -p "$scratch/src" "$scratch/docs"
printf '%s\n' 'fn implementation() {}' 'fn regression_guard_exists() {}' > "$scratch/src/example.rs"
printf '%s\n' 'An ordinary tracked file.' > "$scratch/PR.md"
printf '%s\n' 'A tracked nested body filename.' > "$scratch/docs/pr.md"
git -C "$scratch" add src/example.rs PR.md docs/pr.md
git -C "$scratch" commit -qm 'fixture root'
root_sha="$(git -C "$scratch" rev-parse HEAD)"

git -C "$scratch" switch -q -c unrelated
printf '%s\n' 'unrelated branch' > "$scratch/unrelated.txt"
git -C "$scratch" add unrelated.txt
git -C "$scratch" commit -qm 'unrelated review lane'
unrelated_sha="$(git -C "$scratch" rev-parse HEAD)"

git -C "$scratch" switch -q --detach "$root_sha"
printf '%s\n' 'integrated head' > "$scratch/integrated.txt"
git -C "$scratch" add integrated.txt
git -C "$scratch" commit -qm 'integrated head'
head_sha="$(git -C "$scratch" rev-parse HEAD)"

header='| ID | Severity | Reviewed SHA / location | Failure sequence | Provenance | Category | First bad / prior ID | Regression or documented guard | Disposition |'
separator='|---|---|---|---|---|---|---|---|---|'
row="| PRX-001 | P1 | $root_sha / src/example.rs:2 | bad input -> weak check -> incorrect success | fix_regression | security-trust | pre-commit lane | \`regression_guard_exists\` | fixed |"
body="$header"$'\n'"$separator"$'\n'"$row"
validator_options=()

expect_pass() {
  local name="$1"
  local candidate="$2"
  if ! printf '%s\n' "$candidate" | (cd "$scratch" && bash "${validator_options[@]}" "$validator" "$head_sha"); then
    echo "expected ledger evidence pass: $name" >&2
    exit 1
  fi
}

expect_fail() {
  local name="$1"
  local candidate="$2"
  if printf '%s\n' "$candidate" | (cd "$scratch" && bash "${validator_options[@]}" "$validator" "$head_sha") \
    >/dev/null 2>&1; then
    echo "expected ledger evidence failure: $name" >&2
    exit 1
  fi
}

expect_pass 'integrated reviewed commit and live guard' "$body"
expect_fail 'unresolvable reviewed SHA' "${body/$root_sha/0000000000000000000000000000000000000000}"
expect_fail 'review-lane commit not integrated' "${body/$root_sha/$unrelated_sha}"
expect_fail 'missing location path' "${body/src\/example.rs/src\/missing.rs}"
expect_fail 'location beyond end of file' "${body/src\/example.rs:2/src\/example.rs:99}"
expect_fail 'stale renamed regression' "${body/regression_guard_exists/old_regression_name}"

# The review driver hands the reviewer the body as `pr.md`, which is in no tree:
# a row located there skips the tree lookup and line count, and nothing else.
in_body="${body/src\/example.rs:2/pr.md:12}"
expect_pass 'finding located in the body as pr.md' "$in_body"
expect_fail 'body location with line zero' "${body/src\/example.rs:2/pr.md:0}"
expect_fail 'body location at a review-lane commit not integrated' \
  "${in_body/$root_sha/$unrelated_sha}"
expect_fail 'body location at an unresolvable reviewed SHA' \
  "${in_body/$root_sha/0000000000000000000000000000000000000000}"
expect_fail 'body location with a stale renamed regression' \
  "${in_body/regression_guard_exists/old_regression_name}"
expect_fail 'pr.md under a directory still needs its file' "${body/src\/example.rs:2/docs/pr.md:12}"
expect_fail 'pr.md with a suffix still needs its file' "${body/src\/example.rs:2/pr.mdx:12}"

# PR433-BODY-PATH-CASE-FOLD: shell settings cannot turn another filename into
# the virtual location. Real tracked paths keep their own spelling and bounds.
for shell_mode in default nocasematch; do
  validator_options=()
  [[ "$shell_mode" == default ]] || validator_options=(-O nocasematch)
  for ending in lf crlf; do
    candidate="$body"
    if [[ "$ending" == crlf ]]; then candidate="${candidate//$'\n'/$'\r\n'}"$'\r'; fi
    expect_pass "$shell_mode/$ending tracked path" "$candidate"
    expect_pass "$shell_mode/$ending canonical body" "${candidate/src\/example.rs:2/pr.md:12}"
    expect_pass "$shell_mode/$ending uppercase tracked path" "${candidate/src\/example.rs:2/PR.md:1}"
    expect_pass "$shell_mode/$ending nested tracked path" "${candidate/src\/example.rs:2/docs/pr.md:1}"
    for path in PR.md:12 PR.MD:12 Pr.md:12 pR.md:12 pr.Md:12 pr.mD:12 \
      docs/pr.md:12 pr.mdx:12 ./pr.md:12 pr.md:0 pr.md:01 pr.md:-1 pr.md:1:2 pr.md:1-2 \
      $'p\rr.md:12' $'pr.\rmd:12' $'pr.\tmd:12' $'pr.\vmd:12' $'pr.\001md:12' \
      $'src/ex\rample.rs:2'; do
      expect_fail "$shell_mode/$ending invalid or untracked location [$path]" \
        "${candidate/src\/example.rs:2/$path}"
    done
  done
  # NUL cannot be held in a shell variable; send its bytes directly to stdin.
  if { printf '%s\n%s\n' "$header" "$separator";
       printf '| PRX-001 | P3 | %s / pr.\000md:12 | a -> b | pre_existing | correctness | — | `regression_guard_exists` | fixed |\n' "$root_sha";
     } | (cd "$scratch" && bash "${validator_options[@]}" "$validator" "$head_sha") >/dev/null 2>&1; then
    echo "expected rejection of an interior NUL ($shell_mode)" >&2
    exit 1
  fi
done

echo 'PR ledger evidence fixtures passed'
