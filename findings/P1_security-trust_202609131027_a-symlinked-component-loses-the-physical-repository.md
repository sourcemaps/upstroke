---
id: PR280-ASCENT-FOLLOWS-WRITTEN-PARENTS-THROUGH-A-SYMLINK
severity: P1
disposition: deferred
category: security-trust
pr: 280
reviewed_sha: 7ab8329df9fe5f48ea75e9893c7179c0e87d0fa4
location: .github/scripts/validate-pr-branch.sh:1416
provenance: pre_existing
first_bad:
guard: a change that gives the anchor ascent git's own physical position after a `false`
---

## Failure sequence

`locate_listing` finds a repository by ENTERING directories and names the listing LEXICALLY, and
its ascent steps to `${anchor%/*}` -- the WRITTEN parent. Git does not: `git -C <path>` chdirs and
resolves the path physically. Where a component of the listing is a SYMLINK OUT OF THE WRITTEN
TREE the two chains part, and the ascent walks up a tree git was never in.

```bash
# the PHYSICAL side: a work tree that ignores a bare repository under it
git -C "$EXT" init; printf 'bare.git\n' > "$EXT/.gitignore"; git -C "$EXT" commit -am seed
git init --bare "$EXT/bare.git"; mkdir "$EXT/bare.git/holder"
: > "$EXT/bare.git/holder/P1_security-trust_202609110931_<the branch's description>.md"

# the WRITTEN side: a plain directory, in NO repository, holding the symlink
mkdir "$W"; ln -s "$EXT/bare.git" "$W/alias"
cd "$W" && bash .github/scripts/validate-pr-branch.sh fix-P1/security-trust_<that description> alias/holder
```

Git answers `false` for `alias/holder` -- it is inside the bare repository the link points at --
and the ascent then asks about `$W/alias`, `$W`, and up the WRITTEN tree, which is in no
repository at all. It runs out of levels, chooses the `filesystem` world, enumerates `holder`, and
**conforms at exit 0**. The physical enclosing work tree `$EXT`, which records nothing under
`bare.git` and would have made the listing the empty set, is never reached.

Measured on this box, all at exit 0 `conforms`:

| revision | `alias/holder` |
|---|---|
| `231c1aad` (master) | exit 0 |
| `04432736` | exit 0 |
| `339a238b` | exit 0 |
| `1a8dd545` | exit 0 |
| `7ab8329d` | exit 0 |
| `3403b7fb` (this round's head) | exit 0 |

It is PRE-EXISTING rather than introduced by `#280` -- but the walk it fails in is the one
`04432736`, `339a238b`, `88d2a801` and `1a8dd545` wrote, and at `231c1aad` the same input conformed
by a different mechanism, a `false` ENDING the walk. The rewrite carried it forward.

**Two witnesses, and the second is the one that names it.**

- **Equivalent inputs disagree.** The same directory spelled physically,
  `$EXT/bare.git/holder`, refuses at exit 1 `git records nothing at ... under it or above`. One
  directory, two spellings, two verdicts.
- **`bare.git/HEAD` decides the verdict.** Renaming it away leaves a directory git no longer reads
  as a repository: exit 1. Putting it back: exit 0 again. Nothing about what any repository RECORDS
  changed, and a file that merely makes a directory LOOK bare moved the answer.

That second witness is exactly the defect `1a8dd545` closed -- its commit message and the gate's
`a plain directory inside a bare repository is no ledger either` fixtures say a `false` is not the
end of the ascent and the `HEAD` file must not decide. That repair holds for a directly spelled
path and does not reach a path that arrives through a symlink.

It was not closed by this round's rooting repair (`a870321d`), and the review that reported both
expected one fix to close both. The anchor for `alias/holder` was already rooted -- it takes the
ordinary `*)` arm and is joined onto `$PWD` -- so rooting changes nothing here. Rooting closed the
`C:/…` case in both of its measured shapes and this one is a different mechanism: not an anchor
with no top, but an anchor whose top is in the wrong tree.

## What the change that takes this up should do

Give the ascent git's own physical position after a `false`, rather than stepping to the written
parent. A `false` means git found a git directory and no work tree over it, and
`rev-parse --absolute-git-dir` then answers where that directory physically IS -- measured here:
`/…/bare.git` from `$W/alias/holder` and from `$EXT/bare.git/holder` alike, and `<wt>/.git` for a
path inside a non-bare `.git`. Continuing from that path's parent lands on `$EXT`, where
`--is-inside-work-tree` is `true`, and both `HEAD` states then refuse alike. This carries a path
GIT resolved and not one a subshell resolved, which is the distinction the function's own header
draws when it says why no resolved path is carried between commands.

Three things it has to settle, and none is a one-line correction, which is why this is filed
rather than fixed in `#280` -- whose round-6 brief scoped it out, and whose ascent has produced a
new defect in three consecutive rounds:

- **Boundedness.** The lexical walk terminates because every step shortens the path. A hop to
  git's answer does not shorten anything, so termination has to rest on a different argument: with
  no `GIT_DIR` or `GIT_WORK_TREE` in the environment the git directory discovered from the anchor
  is at or above it physically, so each hop strictly ascends one physical tree. That guard is
  `repository_above`'s first line and would have to be made a condition of the hop, not an
  assumption of it.
- **A pinned environment.** With `GIT_DIR` exported, `--absolute-git-dir` answers the pinned
  directory wherever the anchor is, and a hop to its parent is a jump to an unrelated tree. See
  `PR280-ENV-EXPORTED-WORK-TREE-REFUSES-A-VALID-CALLER`, which is the same reading in the ascent's
  other half.
- **A path whose resolution is not stable.** `/proc/self` resolves per process, so a path git
  answers with can name a directory that is gone before the next probe. Git exits 128 rather than
  `false` for `/proc/self` today, so the hop is not reached there, but a fixture has to pin that
  rather than leave it to be true by accident.

The fixture it wants is the reproduction above plus its `HEAD`-renamed twin, asserted to refuse
ALIKE, next to the existing direct-spelling ones -- and a legitimate caller beside it: an ordinary
listing reached through a symlink that stays INSIDE the same work tree, which must keep answering
exactly as it does now.
