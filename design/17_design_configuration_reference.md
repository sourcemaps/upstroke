## 17. Configuration reference

Config has two scopes (v2.1, widened 2026-11-23). The **machine level** is one directory, `~/.upstroke/`, and holds what is the operator's rather than a repository's: `pools.toml`, because subscriptions travel with the person — **pools are user-level**, discovered and written by `upstroke connect` — and an optional `upstroke.toml` of settings wanted in every repository on the machine. The **repo level** is the repository's own `upstroke.toml`: **routing and gates are repo-level overrides** on derived defaults. Both settings files are one schema, and the repository's is laid over the machine's and wins wherever both speak ("Two scopes", below). A fresh repo on a fresh machine runs with zero config.

`~/.upstroke/pools.toml` (written by `connect`, hand-editable):

```toml
[pools.claude-max]
kind = "subscription-window"
agent = "claude-code"
window = "5h"
weekly = true
sources = ["signals", "self", "local-logs"]
safety_margin = 0.15
reserve = 0.20                      # headroom for the user's interactive sessions

[pools.copilot]
kind = "credits"                    # "request-pool" on legacy annual plans
agent = "copilot"
sources = ["signals", "self"]
monthly_allowance = "auto"

[pools.local]                       # v0.2
kind = "unmetered"
agent = "aider"
endpoint = "http://homeserver:11434/v1"

[pools.anthropic-api]               # a pool the capacity tracker can ask about (§13)
kind = "api-key"
agent = "claude-code"
monthly_allowance = 200             # dollars: what a reported spend is measured against
usage_provider = "anthropic-cost"   # which documented usage endpoint `capacity --refresh` asks
usage_token_env = "ANTHROPIC_ADMIN_KEY"  # the NAME of the variable holding the key, never the key
```

**What `connect` writes.** The file above is an operator's. The one `upstroke connect` writes is a
persisted format with two readers — `config`, for every `run`, `validate` and `capacity` that loads
pools, and `connect` itself on its next run — and it has this shape:

- One `[pools.<name>]` table per agent whose CLI probed and answered discovery, in registry order,
  **named after the agent** (`claude-code`, `copilot`, `codex`) rather than after a plan: discovery
  does not establish a tier, and the pool is the operator's to rename. An agent that is not usable
  on the machine gets a comment saying so and no table.
- The keys: `kind` and `agent` always; `window = "5h"` and `weekly = true` for a
  `subscription-window` pool; `sources = ["signals", "self"]`, the two sources v0.1 reads, never
  `local-logs` or `provider-endpoint`; `safety_margin = 0.15` and `reserve = 0.20`, §13's defaults.
  Then the operator's own keys, written only when the existing file held them under the same pool
  name: `profile`, `monthly_allowance` as a number (`"auto"` is the reader's default and is not
  written), `endpoint`, and the three usage keys, `usage_provider`, `usage_token_env` and
  `usage_account`. `connect` invents none of the six. An allowance must be a positive,
  finite number or the string `"auto"`. An invalid allowance produces a warning and leaves the
  discovered `auto` default; valid `profile` and `endpoint` values are still carried. The usage
  keys are carried as a set or not at all: where the reader would refuse them (below), a warning
  says why, without repeating a value that may be a credential, and none of the three is written.
  `sources` stays the two connect discovers; the reader takes a named `usage_provider` as the
  `provider-endpoint` source's opt-in, so the written file need not list it.
- Comments carry what discovery found: a header naming the version, the write time (RFC 3339, UTC)
  and where the model roster came from; per agent, the auth state — one of *signed in*, *NOT signed
  in* and *could not be determined*, never the second for the third — each discovery note on its
  own line, and a sentence saying the `kind` is a default where the CLI could not say.
- Strings and table keys are TOML-encoded to preserve their values. Finite positive allowances
  are written as numbers the configuration reader accepts. Every comment payload is written one
  line per line with forbidden control characters replaced, so a discovery note cannot become a
  setting or make the file malformed.

**What `profile` does.** A pool's `profile` is the directory handed to the pool's agent as its
credential location: `CLAUDE_CONFIG_DIR` for Claude Code, so the CLI runs on the account logged
in there, and the engine handles no token. The host runner sets it. A worker runs under the
profile of its attempt's pool: the first pool in the file naming the attempt's agent that the
run's signals do not hold exhausted (with every one held, the first whose recorded reset is two
minutes past, or else the first; a retry that resumes its session keeps its pool). Review and
probe processes run under the profile of their agent's first pool in the file, whatever pool an
attempt is bound to. A gate gets no credential location. The value is used as written, with no
`~` expansion, so it must be an absolute path. A profile is supported for Claude Code on the host
runner only. Before any agent CLI is started, the engine refuses a profile on the container runner,
a profile for another agent, a relative path (`~/…` among them), or one whose directory is missing
or not a directory, checking every pool that names one. It also
refuses any profile while `CLAUDE_CODE_OAUTH_TOKEN` is set, because that token overrides a
profile's credentials. A fresh run makes these checks in `prepare_fresh`, before its lease, so a
refused run leaves no run directory and no private half. A resume makes them first in its
pre-flight, under its locks. The refusals are the same, word for word, on both. Pre-flight then runs the agent CLI's capability
probe under the profile and, for Claude Code alone, its sign-in check (`claude --setting-sources ""
auth status --json`), on a fresh run and on a resume, once under each account Claude Code's pools
use, in file order: each profile they name, and the default account when any of them names none or
Claude Code has no pool. It stops at the first account that is signed out or whose check fails to
answer, and that account refuses the run before it starts or resumes; a CLI that cannot say passes.
The check asks with no user, project or local settings loaded, as a worker loads none, so a
credential that only a settings file supplies (an `apiKeyHelper`, say) does not count. An agent
whose pools use more than sixteen distinct accounts is refused before its sign-in is checked.
Other agents' sign-in is not checked at pre-flight, so a signed-out Codex fails at its first
attempt (`PR408-CODEX-SIGN-IN-NOT-PREFLIGHTED`). The legacy
(schema 1–3) coordinator refuses any pool that names a profile.

**What the usage keys do.** They opt a pool into the capacity tracker (§13), and only an operator
writes them. `usage_provider` names which documented usage endpoint `upstroke capacity --refresh`
asks for this pool: `anthropic-cost`, `openai-cost`, `github-ai-credits` or
`github-premium-requests`, spelled exactly; any other value is refused at load, naming the four.
`usage_token_env` is the **name** of the environment variable the tracker reads the credential
from at the moment it asks, never the credential itself, which never belongs in the file. A value
that is not a variable's name (letters, digits and underscores, not starting with a digit, at
most 64 characters) is refused without being repeated, since the usual way to get there is
pasting the key. So is a value with a credential's shape, even where its characters would make a
name: one that starts as a provider's token does (`github_pat_`, `ghp_`, `gho_`, `ghu_`, `ghs_`,
`ghr_`, `sk-`, `sk-ant-` or `xoxb-`, matched exactly, case included) or one longer than 64
characters. A GitHub fine-grained token is letters, digits and underscores, so only the shape
tells it from a name. The refusal names the pool and says the key takes a variable's name, and
`connect` applies the same check, so it does not carry such a value. `usage_account` is the
account, where the provider reports per account (a GitHub login); it is placed in the
request's address, so it is refused unless it is letters, digits, `.`, `_` and `-`. A usage key
with no `usage_provider` is refused, because the likeliest cause is that key misspelled. A
provider with no `usage_token_env`, or with no account it needs, loads with a warning and is
skipped at refresh: neither is a reason to refuse a run. A pool that names a provider has
`provider-endpoint` among its `sources` whether or not the file lists it; `provider-endpoint`
listed with no provider is, as before, parsed and not read. Every amount a provider reports is
measured against `monthly_allowance`, so that key is what turns a report into a ceiling, and it
must be in the provider's units: dollars for the two cost endpoints, AI credits or premium
requests for GitHub's.

**When it is rewritten.** Two comparisons against the file already there, because two questions
are asked. *May* it be replaced: only if both complete TOML documents parse to equal tables, or
`--force` is given; otherwise `connect` refuses, prints the file it would have written, and exits
non-zero. Quoted values retain their whitespace and escapes; equivalent string spellings,
formatting and table order do not cause a conflict. Integer and float values remain distinct.
For example, an older writer's integer allowance `10000000000000000` differs from this writer's
float `1e16` and requires `--force` once. No universal cross-version `Unchanged` behavior is
promised. Malformed TOML, including malformed comments, requires `--force`. A read error other
than a missing file is reported even with `--force`.

There is one roster-upgrade exception: if the existing TOML contains a pool whose `agent` is
`codex`, and the installed CLI satisfies the entire earlier two-model roster and its five
efforts but fails an added model or effort requirement, `connect` refuses even with `--force`.
It leaves the existing file byte-for-byte intact, including all operator settings, and asks
the operator to restore a compatible Codex CLI and retry. This is not evidence that the saved
provider disappeared. Fresh discovery without a saved Codex pool retains the normal unusable-agent
report; genuinely absent CLIs, failures of the earlier requirements and unrelated providers
retain their existing behavior. Once the full roster passes, a deliberate `--force` rewrite
works as below.

*Should* it be rewritten: if anything but the first generated write-time header differs,
comments included, so a login between two runs updates the auth comment while an
unchanged machine leaves the file, and the date on it, alone. `--force` carries `profile`,
`monthly_allowance`, `endpoint` and the three usage keys over from the pools of the same name in
the existing file and replaces everything else; a file that does not parse carries nothing. Invalid allowances are
omitted with the warning described above. Repeated connects using the same writer leave an
unchanged machine's file alone once its settings and nonvolatile content match.

**Frozen across versions** (the owner's rule, r6 answer 4). The pool keys listed above — `kind`,
`agent`, `window`, `weekly`, `sources`, `safety_margin`, `reserve`, `profile`,
`monthly_allowance` and `endpoint` — and the default pool name, the agent's id, are frozen. A
change adds a new key beside the old one, which is still read. The default pool name never
changes, so carrying an operator's keys by pool name keeps working from one version to the next.
The reader still warns on an unknown key and refuses an unknown value. `usage_provider`,
`usage_token_env` and `usage_account` were added beside the ten on 2026-11-23, under that rule,
and are frozen with them.

**Two scopes: the machine level and the repo level.** Every setting below can be written in
either file. The machine-level `upstroke.toml` sits beside the pools file, `~/.upstroke/upstroke.toml`,
and is what a repository with no settings of its own runs under; the repository's `upstroke.toml`
is laid over it. The rules:

- **A table merges key by key**, at every depth, so `[engine] max_per_pool` from the machine and
  `[engine] shell` from the repository both stand, and an inline table such as
  `review = { tier = "frontier" }` merges the same way.
- **Anything else replaces whole**: a scalar, and an array. `[[gates]]`, `[[pins]]`,
  `[[routing.overrides]]` and `notify` are arrays, so a repository that writes any `[[gates]]`
  runs exactly its own and none of the machine's. Appending would make a repository's gates depend
  on a file outside it, with no way for the repository to take one away.
- **The machine-level file must be a valid configuration on its own.** It is read and checked
  first, as the whole of a configuration, and a refusal names it; a `[runner] kind = "container"`
  there with the `image` left to each repository is refused, because a repository with no file
  would run under it. The merged result is then checked and a refusal names the repository's file,
  adding that the machine-level file is applied beneath it, since the cause can be a key that file
  supplied or the two together.
- **What the machine file decided is said.** A run, a `validate` and a `capacity` whose settings
  include a machine-level key the repository left alone warn once, naming both files and each
  such key by its dotted path (`engine.max_parallel`, `gates`). A machine-level file that decides
  nothing says nothing. A warning about a machine-level key names the machine-level file.
- **The machine level is the pools file's directory.** `connect --pools` and `capacity --pools`
  name another pools file, and the settings file, the usage reports and the run index (§13, §15)
  are then looked for beside it. A pools file kept in a repository's own root puts the
  machine-level settings at that repository's `upstroke.toml`: that is one file and one layer.
- **A resume reads both files as they are today**, as it already read the repository's (§15).
  The run's record names only the repository's file.

Repo-level `upstroke.toml` — overrides only; everything below has a derived default:

```toml
[engine]
on_task_failure = "halt"            # halt | continue
max_parallel    = 1                 # >1 runs that many task pipelines at once (schema 4)
max_per_agent   = 1                 # concurrent invocations of one agent; default max_parallel
max_per_pool    = 1                 # concurrent invocations drawing on one pool; default max_parallel
max_merge_repairs = 2               # autonomous generations per original task; then HUMAN
shell           = "powershell"      # gate shell; default = platform native

[interaction]
mode      = "on_block"              # never | on_block | on_milestone
notify    = ["cli", "desktop"]      # + "telegram", "slack" in v0.2
ask_before = { frontier_escalation_over_usd = 5.0 }

[capacity]
scope = "machine"                   # machine | repo: which runs feed the capacity views (§13)

[budgets]
run_usd  = 15.0                     # API-equivalent; omit = unlimited
task_usd = 4.0
# Both ceilings sum REPORTED dollars, so they bound only the routes that report
# any. A run whose implementer is Codex and whose reviewer is Claude Code is
# bounded on the review half alone — the Codex half reports tokens and no price
# (§21), and is bounded by its own subscription window instead. The ledger says
# so with `?`; this comment exists because `--budget 15` otherwise reads like a
# guarantee about the whole run. A token-denominated ceiling is v0.2 capacity
# work if it is ever wanted.

[routing.strategy]
mode = "value-max"                  # conserve | value-max | deadline
spend_down_after = 0.7              # >70% of window left near reset → bias tiers up

[routing]                           # chains are ABSTRACT TIERS — the binder picks models and pools
fix       = { chain = ["small", "mid", "frontier"], attempts_per = 2 }
implement = { chain = ["mid", "frontier"], attempts_per = 2 }
review    = { tier = "frontier", timeout_secs = 5400 }
                                      # independent budget per pass; includes one format re-ask

[routing.effort]                    # optional role-wide standard; outranks pins and tier defaults
implementation = "xhigh"
review = "max"

[[routing.overrides]]                 # at least one of start_at / second_opinion
paths = ["src/auth/**", "migrations/**"]
start_at = "frontier"                 # optional — omit to add a reviewer without raising the floor
second_opinion = "different-vendor"   # binder must add a reviewer from another model family

[[pins]]                            # optional determinism — otherwise the binder chooses
tier  = "frontier"
agent = "claude-code"
model = "claude-opus-4-8"
effort = "max"                      # optional; default is the tier's when no role policy applies.
                                    # Validated at load —
                                    # the provider rejects an unknown level with a 400 mid-turn,
                                    # so a typo would otherwise cost a whole attempt.

[[gates]]
name = "check"
cmd  = "cargo check --all-targets"
timeout_secs = 600

[[gates]]
name = "test"
cmd  = "cargo test"
timeout_secs = 1200
```

**What a settings file refuses.** The same rules bind the machine-level file and the repository's. A mistake that would silently delete a control is an error at load, before a lock, a workspace or a run directory exists; a mistake that only degrades what the run can say about itself is a warning that names the key. So: an unknown top-level section name (anything outside `routing`, `pins`, `gates`, `engine`, `interaction`, `budgets`, `runner`, `capacity`) is an error naming it and the accepted eight, the same treatment as the sections below it — a misspelled section used to disappear whole, with no warning, deleting the largest unit a typo can delete. An unknown key in `[runner]`, `[engine]`, `[budgets]`, `[capacity]`, `[interaction]`, its `ask_before` table, `[routing.strategy]`, or a `[[gates]]` entry is an error naming the accepted keys — `[engine]` included, because `on_task_failure` and `shell` sit in that table beside the ceilings, and `on_task_failur = "continue"` left to a warning is a halted run the operator asked to continue; an unrecognised `kind`, `on_task_failure`, `mode`, `second_opinion`, `[capacity]` `scope` or effort value is an error, while an unrecognised `shell` warns and takes the platform default, and an unrecognised `[routing.strategy]` `mode` warns and is echoed as written; a zero ceiling, timeout or `attempts_per` is an error, as is a gate `timeout_secs` the run record cannot hold (more than 18,446,744,073,709,551 seconds, since `run_started` records it in milliseconds as a 64-bit count), a budget ceiling that is not a positive finite number of dollars, or an `ask_before` threshold that is negative or not finite; and two `[[gates]]` entries may not share a `name`, compared without regard to ASCII case because a case-insensitive filesystem keeps one log file for both, since the name is what a gate's log file and its failure report carry. A resume reads `[[gates]]` by what its log records (§15): a run whose log records its gates takes them from the record, so today's section is compared with them and never refused over — every shape above is a warning naming the recorded gates as what runs — while a run whose log predates the gate record settles its gates from today's file on that resume and reads the section exactly as a fresh run does. Nothing else in the file reads differently on a resume except the `[engine]` ceilings, which warn there rather than refuse. `upstroke capacity` reads only `[routing.strategy]`, `[capacity]` and the pools file: a file that parses as TOML but is refused over any other section still gives the report, with the refusal as one warning that names the settings file read; a refusal in a section it reads still fails it, and so does a file that does not parse as TOML at all (a syntax error, a key repeated within a table, bytes that are not UTF-8), whose sections cannot be read apart. `capacity --refresh` uses only the pools file but takes that same reading first, because the report follows it in the same command: what would fail the report stops the command with no provider asked and nothing stored.

**Which CLIs a schema-4 run needs.** Before a schema-4 run commits anything, it probes the shell and every agent its chains and its review plan name. A probe that fails stops the run's creation at P4, before its commit record, naming the CLI. So every CLI the review plan names must be installed where the run executes its roles: on `PATH` for the host runner, in the image for the container runner.

The review plan names its primary reviewer: the review tier's `[[pins]]` entry, or the catalog's binding for that tier. Where the primary's model family is in the catalog, and unless `cross_family = false` (below), it also names an alternative from another family whose adapter this build carries: the first such catalog entry at that tier, which reviews instead of the primary any task the primary's exact model implemented, unless the task has an explicit second opinion. With this build's adapters, no pins and the default `frontier` review tier, the primary is `claude-code/claude-opus-5` and the alternative is `codex/gpt-6-astra`, so the Claude Code and Codex CLIs must both be installed and the GitHub Copilot CLI is not needed; a `mid` or `small` review tier pairs with a Copilot-served model instead.

The source of truth for the Codex roster is `CATALOG` in `src/catalog.rs`, read by the probe through `catalog::known_models("codex")`: in catalog order, `gpt-6-astra`, `gpt-5.6-sol`, `gpt-6.2-sol`, `gpt-6.2-astra`, all OpenAI-family frontier models. The two 6.2 entries are appended, so catalog ordering still selects `gpt-6-astra` as the default alternative above. That ordering does not establish adapter availability: the Codex probe in `src/agent/codex.rs` reads the CLI's local model catalog (`codex debug models`) and refuses unless it lists every one of those models, each with all Upstroke-supported efforts (`Effort::ALL` in `src/ir.rs`: `low`, `medium`, `high`, `xhigh`, `max`). That holds wherever Codex is probed, including a run that pins only `gpt-5.6-sol` and a default schema-4 run that probes Codex for its alternative reviewer without any 6.2 pin. A missing 6.2 model or effort therefore fails that probe even though catalog ordering has not changed the selected reviewer.

`[routing] review = { cross_family = false }` names no alternative, so the alternative stops being a reason to probe its CLI; a CLI that any remaining binding needs — the primary's own, an implementation chain's, a `second_opinion` override's — is still probed and required. The alternative can share its CLI with the primary (a `mid` review tier pinned to `copilot/claude-sonnet-5` has `copilot/gemini-3.1-pro` as its alternative, both served by the `copilot` agent), and then that CLI is still probed for the primary. The switch is the way to keep reviewing when the other family's account is out of capacity, short of `enabled = false`. The primary then also reviews the tasks its own model implemented; where a task with no explicit second opinion can run on that model, the plan warns once, naming the primary and those tasks. Unset, or `true`, it is the behaviour above; on a task-kind entry it is refused. It turns off only that opportunistic alternative: a `[[routing.overrides]]` entry's `second_opinion` is an explicit request, still resolves to another family, and is still probed and required.

This is the plan a run resolves when it starts. A run records its review plan, and a resume keeps the reviewers recorded, though a recorded Codex reviewer still meets today's Codex probe. Only a sequential log that predates the review record resolves its plan again on resume, from today's config and this build's catalog.

Sequential preflight distinguishes this roster upgrade from pre-existing unavailability. If the CLI passes all earlier probe requirements, including both `gpt-6-astra` and `gpt-5.6-sol` with every required effort, but lacks an added model or effort, it refuses before work or any durable review-plan change. This protects schema-3 recorded plans and schema-2 plans both with and without a review record: the latter are still re-derived, but no downgraded plan is persisted. Restore a CLI whose local `codex debug models` advertises the full roster and all five efforts, then retry the original resume; the independent reviewer remains available to the original plan. Classification comes from the parsed local catalog, never arbitrary stderr text.

For unrelated pre-existing probe failures, including a genuinely absent CLI or a failure of the earlier two-model requirements, the sequential optional-alternative behavior is unchanged: preflight warns and drops the alternative, and the primary may review work its own model implemented. This fallback does not cover Codex needed by an implementation chain, the primary reviewer or an explicit second opinion: those agents must pass their probes. A schema-4 run does not drop an unavailable alternative, because its recorded probe set binds its resume too. The regression tests use isolated configured CLI stubs, temporary pools and synthetic persisted run records; they exercise refusal and a full-roster retry without provider accounts.
