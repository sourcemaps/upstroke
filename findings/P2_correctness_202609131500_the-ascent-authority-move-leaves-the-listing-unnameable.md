---
id: PR280-ASCENT-AUTHORITY-MOVES-OFF-THE-WRITTEN-CHAIN
severity: P2
disposition: deferred
category: correctness
pr: 280
reviewed_sha: 8a6c9ce882c7eab9610b1d1f8cced9af4e992ee9
location: .github/scripts/validate-pr-branch.sh:2047
provenance: fix_regression
first_bad: 04432736
guard: a change that gives the gitlink ascent a name for a listing reached through a link out of the written chain
---

## Failure sequence

The gitlink ascent moves authority to a work tree ABOVE the listing's own root when nothing in the
listing's own repository names it. The listing must then be named in THAT index, and the name is
taken from the caller's own components -- rule 6 of the walk invariant, and the reason a `findings`
that is a symlink to `saved-findings` is still recorded under the name the caller wrote. Where a
component of the listing was a symlink OUT of the written tree, the caller's chain does not reach
the new root, no prefix matches it, and the run is refused. The same directory spelled physically
conforms.

Executed here, git 2.43, on this box:

```bash
mkdir -p EXT/project W
git -C EXT init -q
printf 'x\n' > EXT/project/P2_correctness_202609100001_x.md
git -C EXT add -A && git -C EXT commit -qm base    # EXT records project/P2_...x.md
git -C EXT/project init -q                         # project is now its own repository
ln -s "$PWD/EXT/project" W/link
```

| spelling | result |
|---|---|
| `cd EXT && validate-pr-branch.sh fix-P2/correctness_x project` | **exit 0, `conforms`** |
| `cd W && validate-pr-branch.sh fix-P2/correctness_x link` | **exit 1**, refused, not judged |

Both name one directory. `project`'s own index records nothing, so `enclosing_work_tree` reaches
`EXT`, `records_path EXT project` answers yes, and authority moves to `EXT`. The physical spelling's
prefix chain holds `EXT`; the symlinked one holds `/`, `W` and `W/link` and holds nothing that is
`EXT`, so the prefix match fails.

The refusal is a narrowing and not a false green: nothing is answered out of the wrong index, and at
`231c1aad` there is no `enclosing_work_tree` at all, so both spellings were answered from
`EXT/project`'s own index and agreed. `04432736`, the commit that added the ascent, is where the
two spellings began to differ.

This round made the refusal say which of rule 6's two shapes it is, because the remedies differ:
before the move the caller's spelling never named its own listing root and respelling fixes it;
after the move the root is one the WALK chose and there is nothing for the caller to respell. The
message that stood there told the caller to respell in both cases.

## What the change that takes this up should do

Give the ascent a name for the listing in the enclosing index that is neither the caller's
unreachable chain nor the link's target. Two shapes were considered and **both reopen something
this pull request closed**, which is why this is filed rather than repaired here:

- **Name it physically, from `subject` relative to `enclosing_root`.** Those are both
  `--show-toplevel` answers, so the segment is always computable. But it is the link's target, and
  rule 6 exists because that is not the name anything records the listing under. Executed here on a
  repository recording `alias` at mode 120000 pointing at `real`, with `real/P2_..._x.md` recorded
  underneath:

  | listing | result |
  |---|---|
  | `alias`, the caller's name | exit 1, `git records 'alias' as mode 120000, which is` neither a file nor a directory |
  | `real`, the link's target | **exit 0, `conforms`** |

  So naming the listing physically turns a 120000 the index records into a ledger with a finding in
  it. That is the `loop` false green `04432736`'s own fixture pins, one level further out.
- **Walk the physical segment through `recorded_tree` first.** It does not separate the two:
  `git ls-files -s -- ':(literal)real'` in that same repository returns the entry, so the enclosing
  index does record entries under the target, the check passes and the wrong answer is returned
  anyway.

A third shape -- resolve the caller's chain physically and match on that -- would undo the inode
match the same commit introduced and is not a narrower change than either.

Whichever is chosen, it wants a fixture with the listing reached through a link out of the written
tree AND one reached through a link that stays inside it, because only the first reaches the
defect.
