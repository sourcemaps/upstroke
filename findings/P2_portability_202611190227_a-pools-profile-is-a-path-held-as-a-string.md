---
id: PR386-PROFILE-PATH-STORED-AS-STRING
severity: P2
disposition: deferred
category: portability
pr: 386
reviewed_sha: 8ad0af365f43959f951d6d0f4a7dd444762b8911
location: src/capacity.rs:125
provenance: introduced_by_feature
first_bad: 8ad0af365f43959f951d6d0f4a7dd444762b8911
guard: the next change to Pool
---

## Failure sequence

Found by the frontier review pass on #386 at `8ad0af36`, by reading the code.

1. The pools file's `profile` is parsed into `Pool::profile`, which is an `Option<String>`
   (`src/capacity.rs:125`). It stays a string for as long as the pool is carried: `connect`
   carries it across a rewrite as one (`src/connect.rs:219`), and `Pool::describe` displays it as
   one (`src/capacity.rs:155`).
2. It becomes a path only at its use sites, and each makes the conversion itself:
   `ProfiledPool::of` for the pre-flight refusals (`src/engine/topology/preflight.rs:51`), and
   `profiles` for the profile the wrapper stamps on each request
   (`src/engine/topology/profiled.rs:25`). Both call `PathBuf::from` on the string as written.
3. So the directory pre-flight checks and the directory the agent runs under are the same only
   because two separate conversions happen to agree. The type does not make them agree. A change
   that alters one of them, such as `~` expansion, a base directory or a Windows-specific
   spelling, has to find and alter the other as well. Nothing fails if it does not, and then
   pre-flight passes one directory while the agent runs under another.

## Note

No failure is known. A TOML string is UTF-8, so `PathBuf::from` converts it losslessly on every
platform, and the two conversions produce the same path today (inferred). `findings/PROCESS.md`
§8 carries no `MUST` that this violates. It is filed so that the next change to `Pool` stores the
profile as a path once, at parse time, instead of adding a third conversion.

## What the change that takes this up should do

Make `Pool::profile` an `Option<PathBuf>`, converted once where the pools file is read. The use
sites then take the path they are given. `connect` and `describe` turn it back into the string
it was read from. A value read from TOML is always UTF-8, so that conversion cannot fail.
