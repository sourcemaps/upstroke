---
id: PR329-THE-GIT-FLOOR-TEST-HELPER-SPLICES-PATHS-INTO-SHELL-TEXT
severity: P3
disposition: deferred
category: portability
pr: 329
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/workspace.rs:1872
provenance: pre_existing
first_bad:
guard: the change that next touches src/workspace.rs's git-floor test (execution_prerequisites_ask_git_its_version_and_refuse_2_40 and its helper git_2_40_prerequisites_helper), or a standards §9 sweep of test code, whichever comes first
---

## Failure sequence

Reasoned from the source; not executed. On a Unix host, under a legal temporary directory whose name carries an apostrophe:

    TMPDIR=/some/where/reviewer's-temp; run
      workspace::tests::execution_prerequisites_ask_git_its_version_and_refuse_2_40
    -> it runs workspace::tests::git_2_40_prerequisites_helper as a subprocess; the helper's temp_repo("git-floor")
       roots its scratch tree under std::env::temp_dir() (src/rundir/scratch_tree.rs:847), so its repository, its
       bin directory and its asked file all carry the apostrophe
    -> the helper writes its stub git by splicing asked.display() and the real Git's path into the program text
       inside single quotes: printf '%s\n' "$*" >> '{asked}' … exec '{real}' "$@"
    -> the apostrophe ends the quoting, so the stub no longer says what it was written to say (expected: a parse
       error or a write elsewhere; the exact shell outcome has not been observed)
    -> the stub does not answer "git version 2.40.0" as designed, or does not record what it was asked
    -> the helper fails an assertion or panics reading asked, and the parent test fails on a valid path before it
       measures the 2.40 floor it exists to measure

A second face, also reasoned: a temporary path with a component that is not UTF-8. The helper builds PATH from
bin.display() and the stub's text from asked.display(), both lossy, so PATH's first entry names a directory that does
not exist (the system's Git answers instead of the stub) or the stub writes to another path. The test then fails, or
measures the wrong Git. The helper also reads PATH with env::var(...).expect("a PATH"), which panics on a PATH that is
not valid Unicode.

The test and its helpers are #[cfg(unix)]. Test-only: no production path builds this script. At 5c222ff2 the parent
test is at src/workspace.rs:1851, the helper at :1872 and the child at :1931; at ed4e70ed and 1401e0a3 they are at
:1927, :1948 (the stub at about :1969) and :2007.

## What the change that takes this up should do

Pass the stub's paths to it as data, never inside its program text: environment values the stub reads in double
quotes, or positional parameters. Build PATH with std::env::join_paths over native paths, reading the inherited PATH
with env::var_os. Witness it under a temporary directory whose name carries ' (and, on Linux, a component that is not
UTF-8), with a catching reversal. The B-I9-1 repair of B-W924-R2's gate-role fixture in src/engine/tests.rs (bc855064)
is the model.
