---
id: PR409-CONFIG-NOTES-DESCRIBE-THE-OLD-LOADING-CONTRACT
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 409
reviewed_sha: 675fc38ee2be92260b75fd947708057ee9f98b40
location: docs/internals/config.md:9
provenance: introduced_by_feature
first_bad: 61ab575dee162cd245fc511e4c7d4bb10e335637 (the sentences were true on master; #409's machine-level settings layer and its [capacity] section made them stale)
guard: the next change to src/config.rs's loading or its top-level sections, or the next docs PR that edits docs/internals/config.md or docs/internals/config/read.md
---

## Failure sequence

From #409 round five's regression pass:

    a reader takes `docs/internals/config.md`'s module overview at its word
    -> it says only two optional files exist, the repo-level `upstroke.toml` and
       `~/.upstroke/pools.toml`, and that both missing falls back to derived defaults silently
    -> with neither present, a valid machine-level `~/.upstroke/upstroke.toml` now supplies gates,
       routing and budgets with an inheritance warning (#409's settings layer, `src/config/layers.rs`,
       described later in the same note and in `design/17`)
    -> the overview contradicts the code and the note's own layering section

The overview, at `docs/internals/config.md:9`:

```text
Two optional files: repo-level `upstroke.toml` (routing overrides, pins,
strategy) and user-level `~/.upstroke/pools.toml` (capacity pools, normally
written by `upstroke connect`). Both missing is the normal fresh-repo case
and falls back to derived defaults silently.
```

The note's own layering section, at `docs/internals/config.md:760` (under `load_captured_with`):

```text
Since 2026-11-23 this is also where the two scopes meet (§17). With no
machine-level file captured it is what it was: the repository's file,
interpreted. With one, three things happen in an order that is the design:
```

`design/17_design_configuration_reference.md:138` states the same: "**Two scopes: the machine level and
the repo level.**", with the machine-level file at `~/.upstroke/upstroke.toml` "what a repository with no
settings of its own runs under", and, at line 156, "**What the machine file decided is said.**" The warning
is built from `layers::supplied` (`src/config/layers.rs:52`).

Related: the section count. At `docs/internals/config.md:19` and its line 21:

```text
The top level of `upstroke.toml`. Its seven fields are exactly the sections
`design/17` documents for the repo-level file, so an unknown section name is a
typo and is refused naming it and the accepted seven. Before this attribute a
```

At `docs/internals/config/read.md:72`, the sentence running from line 71:

```text
whichever of this file or `src/config.rs` swept first): the struct now denies
unknown fields, the refusal names the typo and the accepted seven sections, and
```

`RawRepoConfig` now has an eighth field, at `src/config.rs:37`:

```rust
    capacity: Option<toml::Value>,
```

and `design/17_design_configuration_reference.md:236` lists eight names (`routing`, `pins`, `gates`,
`engine`, `interaction`, `budgets`, `runner`, `capacity`) and "the accepted eight".

Master (`4c9c566b`) carries the same sentences (config.md lines 9-12 and 19-21, read.md line 65), and there
they are true: master's `RawRepoConfig` has no `capacity` field, and `src/config/layers.rs` arrived with
`61ab575d`, which added it and the machine-level layer. The contradiction is the regression pass's reading;
no test, reproduction or mutation was run, in the review or here. That the refusal message itself names
eight at this head is inferred from `design/17:236` and was not run.

## Why it is accepted

- Unwitnessed, and P3: there is no failing test, reproduction or mutation witness; it is a note's wording.
- It carries no `MUST`.
- The regression pass found no executable regression and passed the full suite at the head.
- Round five is #409's last under r51 step 6, whose bound files up to three such P3s `accepted-risk`.
- The layering section later in `docs/internals/config.md` (line 760 on) and `design/17` (line 138 on)
  already state the machine-level file, so a careful reader is told.

## What a change that takes this up should do

- Rewrite the overview at `docs/internals/config.md:9` to name the three files (the repository's
  `upstroke.toml`, the machine-level `~/.upstroke/upstroke.toml`, and `~/.upstroke/pools.toml`) and say
  which absences are silent and which warn, matching `design/17`'s "Two scopes" rules.
- Say eight sections, naming `[capacity]`, at `docs/internals/config.md:19` and its line 21, and at
  `docs/internals/config/read.md:72`.
