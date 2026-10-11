---
id: PR313-ADAPTER-DENY-RULES-MISS-THE-WORKTREE-GIT-POINTER
severity: P2
disposition: deferred
category: security-trust
pr: 313
reviewed_sha: 604d8139749c907e9d0071e4a78438d3cc178eb4
location: src/agent/claude.rs:284
provenance: pre_existing
first_bad:
guard: the Claude adapter's own sweep; `src/agent/claude.rs` is outside the pull request that found this
---

## Failure sequence

the Claude adapter's deny list names `Write(.git/**)` and `Edit(.git/**)` and no rule for the entry `.git` itself -> every engine checkout is a **linked** worktree, whose `.git` is a regular file at the checkout root and not a directory (`runner::container::view::resolve`'s `dot_git_is_file`, and `WorkspaceManager::worktree_git_dir` reads it as a `gitdir:` file) -> whether an agent's file tools may rewrite that file depends entirely on whether the matcher reads `.git/**` as covering the path `.git`, which nothing in this tree measures or pins -> a rewrite of that one file redirects every later Git child the engine runs in that checkout (`PR313-GIT-DISCOVERY-REDIRECT-INSIDE-THE-STATED-BOUNDARY`), which is strictly more than the "cannot rewrite git config" the adapter's own test says the rule is for

## What the change that takes this up should do

Measure the matcher rather than reason about it: run the adapter's own settings against a
path of `.git` and record whether `Write(.git/**)` denies it. If it does not — the ordinary
glob reading — add the rule for the pointer itself (and its `**/` form, since a task
worktree's checkout is the tool's working directory but need not be its only one) and pin
it in `permission_settings_protect_the_permission_files_themselves`, which today asserts
only that the deny list *contains* the string `.git/**` and would pass unchanged with the
pointer writable.

What this is **not**: an OS boundary, and this finding does not claim one. `design/15`
records that adapter deny rules are "defence in depth, not an OS boundary" and that a
repository-authored gate command already executes as the Upstroke user. The reason it is
still a P2 is that an `Edit` profile's shell access is restricted to the configured gate
commands, so for a tool-mediated write the deny list is the layer that is supposed to hold,
and the one path whose rewrite redirects the whole repository is the path it does not name.
