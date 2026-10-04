## 15. Event log, resume, run layout (P6)

The durable run artifacts are **split in two**, by who is allowed to read each half. v0.2 adds a third, disposable execution root that contains code but no authority:

```
<repo>/.upstroke/runs/<run-id>/     # run-id = ULID — the ops surface
    events.jsonl                  # append-only source of truth
    plan.normalized.json          # the frozen plan this run executes
    artifacts/                    # conventions-brief.md, decisions-record.md, contracts
    questions/<question-id>.json  # rendered question payloads for notifiers
    answers/<question-id>.json    # answers dropped by `upstroke answer`
    run.lock                      # advisory; OS-released, so a crash leaves nothing stale
    report.json                   # projection of the log for humans; never read back
~/.upstroke/runs/<run-id>/          # agent-authored — outside every agent's reach
    transcripts/<task>-<attempt>.json
    reviews/<task>-<attempt>-review.json
    settings/<task>-<attempt>.json    # the per-attempt permission surface
    gates/<task>-<attempt>-<gate>.log
    gate-worktrees/                    # synced intents + disposable exact snapshots
~/.upstroke/workspaces/<repo-key>/<run-id>/  # v0.2; exact path recorded on run_started
    tasks/<task>-<generation>/          # detached linked worktrees
    merge/                              # detached integration staging worktree
upstroke.toml                       # repo-root config, checked in
```

A fresh sequential run reserves the global private root and its repository's
public root with exclusive directory creation before creating skeleton files,
opening the event log or entering early-error cleanup. An occupied root refuses
the new run and preserves the existing run's contents, including when two
repositories request the same run id. If public reservation fails, creation
removes only its newly reserved empty private root and reports a failed removal.
Skeleton creation failures retain partial directories for inspection. Resume
keeps its idempotent skeleton creation; schema-4 creation keeps its separate
marker and reciprocal-owner protocol.

New ULIDs retain the 48-bit timestamp and 80-bit suffix layout. The suffix is
the first ten SHA-256 bytes over `upstroke.ulid.v2` followed by a zero byte and
the big-endian timestamp, process id and nonce, with widths 64, 32 and 64 bits.
The separate fields remove the old XOR cancellation between a pid and nonce.
This changes newly generated values, not the spelling or interpretation of
persisted ids. It does not promise unpredictable or collision-free names;
exclusive allocation supplies the fresh-run ownership check.

Test scratch trees use a separate, test-only ownership token. Exclusive
directory creation is followed by retaining a directory handle. Checked use
and reclaim compare the current root's filesystem identity with that handle;
an observed replacement refuses and never becomes the token's claim through
disarm, failed reclaim or rearm. Once successful removal closes the handle, a
failed absence observation leaves a consumed claim that can report failure or
confirm later pathname absence, but cannot reopen a replacement for deletion.
A recursive remover's child-level `NotFound` is not proof of root absence.
Pathname absence also does not prove that an original directory moved elsewhere
was deleted. These are checks at operation boundaries, with the mkdir-to-open
and subsequent check-to-use intervals stated explicitly. They do not provide
continuous isolation against an active same-user writer or protect every child
path from such a writer.

The split keeps transcripts and reviewer records out of ordinary workspace reads, but the shipped host runner does **not** make the public half authoritative against hostile candidate code. Adapter deny rules reduce direct agent-tool access; they are defence in depth, not an OS boundary. Repository-controlled gates execute candidate build/test code as the Upstroke user and can discover the source worktree and modify `.upstroke`. A host-run event log is therefore an operational recovery record for trusted repositories and plans, not a tamper-resistant attestation. Moving coordinator authority outside every role mount and enforcing that with the external/container runner is a blocking backlog item before any stronger claim; use a dedicated OS account or VM for untrusted input.

The v0.2 execution root is deliberately non-authoritative. A container receives only its role's one worktree mount; it never receives the public log, sibling worktrees, or private artifacts. On the host runner the agent permission surface remains the boundary and gate code is not OS-confined — the reason the container runner exists. Worktree disappearance is recoverable from events and internal refs, and cleanup follows a terminal event rather than creating one.

The execution root is created only when the managed base is a real directory, the chain from the authorized private root down to the root carries no symlink or reparse point and no regular file, the canonical root is inside no repository worktree, and no foreign worktree is inside it. A run id is one plain path component, so the path recorded above is the only one it can name. Every create, reclaim and delete revalidates all of that before entering its effect funnel, and re-checks the chain inside the funnel, after its before-hook and immediately before the effect.

Current host-process crash containment is deliberately platform-specific. On Unix, ordinary descendants remain in an isolated process group and a separate cleanup reaper retains the run's cleanup lease if the conductor is killed, as does every `git update-ref` the engine spawns for as long as that child lives, so a resume cannot begin while a ref write of the dead run is still in flight; code that deliberately daemonises out of that group remains outside the host-runner contract. A container invocation is contained on Unix by the container runner's own cleanup reaper, armed before that runner's first container starts and holding no cleanup lease: if the conductor is killed it kills and removes every container the runner started, and it is cancelled only once each of them is established gone; Windows has no such reaper, and a killed conductor's containers are reclaimed by the next write command's startup census. On Windows, each command is created suspended, assigned to a private kill-on-close Job Object, and only then resumed. Direct-child success and timeout both terminate and boundedly observe that job empty; abrupt conductor death closes its non-inheritable handle and lets the kernel terminate ordinary descendants. PID scanning and `taskkill` are not part of the ownership protocol. Exact gate/review worktrees likewise record and sync a private intent before `git worktree add`; resume reclaims every such registration before it switches branches or dispatches another worker.

**A registry another process is writing.** *In force for the topology's registry accesses, as
`reviews/2026-10-01-pr11-follow-up-b-record.md` designs them (§8, over §7, §6 and §5) and its implementation section
records. It needs no packet change. The frozen legacy engine's accesses are a separate change, follow-up D, and the
owner's decision.*

**The defect.** Every checkout of a repository registers its linked worktrees in one shared store,
`<common git dir>/worktrees/`, and Git writes a registration one file at a time.
- Every enumeration of the store dies on an entry half written, or prints a record for it that no
  reader can use: `git worktree list`, the sibling scan inside `git worktree add`, the engine's own
  scans.
- `git worktree prune` decides to delete an entry it finds before the entry's `locked` file exists, and
  deletes it later without looking again, even after the add has locked it and gone on. `git gc` runs
  that prune, and so does Git's automatic maintenance after a commit, a fetch or a merge in any
  checkout: through gc when gc is due, and on Git 2.55 whenever an entry looks prunable.
- The worktree lock excludes a second engine only from the same checkout, so two runs in two linked
  checkouts of one repository raced in the store
  (`PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`). In an attempt the lost race ended the
  command resumably. In a merge verification it was foreign Git state: it spent one of the candidate's
  deferrals, or parked the candidate at `max_defers`.

**Every registry access is attempted again.** Every registry access the workspace manager makes is
one attempt: a list together with the parse of its output and, where the manager compares the
worktrees it lists, the resolution of each path the list names; an add; or one of the manager's
scans.
- A failed attempt is attempted again, after a short backoff, until the access's deadline. Nothing
  reads the store, Git's message or a file's timestamps to decide why it failed.
- A listed path that cannot be read fails its attempt, as a list Git could not finish does. On
  Windows a checkout that another removal has deleted while some handle on it is still open answers
  "access denied" to every open until the last such handle closes, and nothing in the error tells it
  from a path the filesystem denies. A path the resolution reads and refuses, such as a link with
  nothing behind it, is refused at once.
- An add first makes its destination as an empty directory. Git takes a destination over only after
  its own registry steps, and on any later failure removes it with its junk. So a failed add whose
  destination is still an empty directory the access can remove holds nothing to lose, and is attempted
  again.
- Otherwise Git may have taken the destination over, and nothing outside Git can tell a checkout that
  cannot be made from a prune that deleted the add's own registration: the end states are the same.
  The access refuses at once, resumably, as a registry refusal, and never returns that failure as Git
  state. So a verification whose snapshot cannot be made, for a path, a filter or a missing object,
  stops resumably and spends no deferral, and the run stops at it again until the cause is repaired.
- After a successful prevalidation — the add's gate, whose list of the registry is an access of its
  own — a destination that cannot be made, or that is not an empty directory when the add's access
  begins, is the add's own failure. It is returned as Git state at once, before `git worktree add`
  runs, so the registry's state cannot reach that answer. A store the gate's list fails on refuses as
  the registry's first, whatever is at the destination.
- What outlasts the deadline, ten seconds, refuses resumably, as a registry refusal, never as Git
  state a verification could defer or park a candidate on: contention, a registration a dead writer
  left torn, and any other fault of the store, such as a store nothing can write, a registration
  Git cannot list, or a listed path that stays unreadable. A failure the access cannot decide, such
  as a destination holding something the add did not leave, refuses at once.
- The deadline covers the access's waits, its retries and the start of every attempt; the attempt
  after a backoff the deadline cut short is made, at the deadline, and is the last. It does not bound
  the last Git command already running, which a filter, a large checkout or a slow filesystem can
  extend, nor the short decision after it.
- On the topology coordinator a wait answers the coordinator's messages instead of sleeping. A wait
  that answers one that ends the command, such as a shutdown, ends the access there, with no further
  attempt, and the transition it waited in: nothing further is appended, published or spawned for
  it, and the run is resumable (the follow-up B record's §9.16).

**Within one process.** The registry lock no longer serialises registry access: retrying makes each
access safe against another's half-done work, whichever process or thread does it. The lock is held
shared by adds and alone by the repair that removes torn registrations, which must not read an add of
the same process in flight as a dead add's residue.

**No engine prune.** The manager's removals delete their own registration directly, and the store
itself when that leaves it empty. They never run `git worktree prune`, which deletes another
process's add in flight. A removal that finds no store at all removes an empty directory at its slot:
what a coordinator killed after making an add's destination leaves.

**What stays outside it.**
- A coordinator killed inside a Git write leaves that write's processes running on Unix. When its
  resume recreates the slot, they can still act on the slot's paths. That is
  `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`, a separate finding
  with its own change before G6, and this access does not address it.
- An agent's own Git on the host runner and the user's Git in any checkout are attempted past as
  writers, and their own commands are theirs.
- A prune that no engine process starts deletes an add's registration in one of two ways. Such a prune
  is a host agent's, the user's or an IDE's, or Git's automatic maintenance after a commit, a fetch or a
  merge in any checkout.
  - So that the add fails, after Git took its destination over: the access refuses that, resumably.
  - Without failing the add: once Git has written the checkout, the add returns Ok whether the
    deletion lands before it returns or after, and no access can see that. The checkout is left with
    no registration, or part of one, its Git commands fail or misread it, and a verification, a gate,
    a review or a recovery there can reach a durable outcome that is wrong for valid work.

  That is `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, a separate finding that
  blocks G6 until it is closed or the owner rules on its scope.
- A writer that died mid-write leaves an entry that stays torn, and the access that meets it refuses
  resumably. Its own run's resume repairs it when Git can still list the store. When Git's listing
  dies on that entry, the resume refuses before any repair. The operator then removes that
  registration directory and the checkout it names, once no Git process is writing it, and resumes.
- The frozen legacy engine's registry accesses do not take this access, and its coordinator discards
  an attempt's output on any error. Both are follow-up D's
  (`PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`), a separate change
  under the owner's decision that calls this access.

**When a Unix helper does not start.** The cleanup reaper and the job-control guard are forked before any agent exists, and each acknowledges its own startup within a fixed budget. A launch that does not see that acknowledgement fails, ends the helper with one `SIGKILL` and a **bounded** wait — by number, or through the identity the next paragraph describes — and reports what those two calls answered, alongside how long it waited, that budget, the descriptor ceiling the helper was closing against, and how the wait ended: on the helper's own report of the setup step that refused and the error it left, on the acknowledgement pipe closing with no report, or on the budget elapsing with nothing on the pipe. A helper that cannot finish its setup writes that report on the acknowledgement pipe it already owns before it ends, and the wait ends the moment the helper ends on every supported platform. On macOS the wait is a `select`, because `poll` on the FIFO the channel is built from never reports the writer's close. The point of reporting these is one distinction: a helper that had **already ended itself** before the signal, whose report or exit status names which of its own setup steps refused, against one that was **still running** and had to be killed, which says it was still working when the budget ran out. Nothing else is claimed. **The wait after the signal is bounded, and a helper still there when it runs out is left behind.** The wait asks the kernel for what it can answer without blocking and asks again until the helper is collected or a second budget of its own elapses; a helper that has not become collectable by then is one the kernel is not ready to hand back — in uninterruptible I/O with the signal pending, say — so the launch reports that it was left for this process's exit to collect and returns, rather than waiting on it. It must return: these launches hold the barrier under which the signal monitor refuses to kill or stop any registered group, so a launch that never returns is every running agent outliving a `SIGTERM` for as long as the kernel takes. Of the waits that end a helper, one is **not** bounded, and deliberately: the end of a run's cleanup reaper that has **acknowledged** CLEANUP or CANCEL, whose exit is what releases the run's cleanup lease the caller is about to act on, so releasing that caller early would let it proceed against a lease still held. A reaper that did not acknowledge CLEANUP — its pipe ended with no answer, it refused, or the request could not be written — is ended with the bounded wait instead, because its caller acts on nothing: the supervisor answers that failure by arming fail-closed termination of this process and returning an error, and a reaper the wait leaves behind holds the lease until it exits, as a reaper does after any coordinator death. The parent asks the kernel nothing about the helper beyond those two calls and the pipe it was already reading, and in particular a pid is never treated as evidence of which process it names — a wait that answers *not collectable yet* is reported as that and never as the helper: while an embedding host may reap this process's children with a wildcard wait, no observation the parent can make establishes that, and the message says only what the pipe carried and what `kill` and `waitpid` returned.

**Which process the end of a helper names.** A pid is not evidence of the process holding it: an embedding host may reap this process's children from its own `SIGCHLD` handler with a wildcard wait, a helper collected there leaves its number free for the kernel to hand to another of that host's forks, and no observation the parent can make tells the two apart. By default the end of a helper is the `kill` and the `waitpid` on its number described above, and that is best effort against such a host: the signal and the wait may reach a process that is not the helper. Upstroke asks no obligation of an embedding host for it, and states this rather than leaving it implied. On Linux an embedder may instead turn the identity path on with `UPSTROKE_HELPER_IDENTITY=1`. With it on, each helper is created by `clone3` with `CLONE_PIDFD`, so the descriptor that names it arrives with the child and there is never a helper this process cannot name; it is signalled with `pidfd_send_signal(fd, SIGKILL, NULL, 0)` and collected through the same descriptor, with `waitid(P_PIDFD, fd, WEXITED | WNOHANG)` asked again until the helper is collected or the bounded wait's budget elapses — except at the one wait above that is not bounded, the end of a cleanup reaper that acknowledged CLEANUP or CANCEL, which is `waitid(P_PIDFD, fd, WEXITED)` and blocks until the reaper has ended. None of these can reach a process the descriptor does not name. Those three calls are the whole of the path: it probes nothing, remembers nothing from one launch to the next, and makes no other call and no call by number. **The trust boundary is the host's syscall policy, beside its platform.** Setting the variable is the embedder asserting that its host's kernel and policy permit those three calls with those arguments — `waitid` with both of those sets of options — and answer them as the kernel does. Upstroke makes none of them on a host where that assertion has not been made, because a policy may kill the caller of a system call rather than refuse it, a process killed for a call takes no fallback, and no answer this process could read beforehand would stay true once a filter is installed. When the assertion does not hold: a policy that kills on one of the calls kills this process, as it would for any other call it forbids; a `clone3` that answers an error — `ENOSYS` on a kernel before 5.3 or under a profile that hides the call, `EPERM`, or a resource error such as `EMFILE` — fails the launch with a message naming the call and the variable, and no helper exists; a signal that answers an error, `ESRCH` included, leaves the helper unsignalled and unwaited, and the message says so; a wait that answers an error, `ECHILD` included, leaves the helper uncollected, and the message says so. Nothing falls back to the number. The path needs Linux 5.4, for `waitid` on a descriptor; it costs one descriptor per helper, held for the helper's life; and a launch with no free descriptor fails where the default would have started the helper. On macOS and the other Unix targets the variable has no effect, and the end of a helper is by number.

**Synced intents.** Each intent file is one JSON object with exactly four string fields, in this
order: `kind`, `slot`, `run_id`, `incarnation`. `kind` is one of `task`, `staging` or `snapshot`.
`slot` is the slot's identifier, `<namespace>/<component>`, the canonical spelling of a slot the
engine validated; it names the slot for whoever reads the record, and the filesystem path is
derived from the intent's file name and the execution root, never from this field. A reader
accepts no other key, no alias for a key or a kind word, no default for a missing field, and no
record whose `kind` disagrees with the namespace of its `slot`; any of those is refused. No code
in the engine acts on a record's contents: reclaim trusts the intent's file name alone, and the
record is provenance for an operator and for any future reader, which this contract binds. The
implementation is `IntentRecord` in `src/workspace_manager/naming.rs`.

**What an exact snapshot is exact against.** A snapshot is exact against the objects the repository
holds, never against the objects `git replace` points at them. `refs/replace/A -> B`
makes Git read `B` wherever `A` is named while `rev-parse` still prints `A`, so a resolve-once check
cannot see it: measured on git 2.43, `commit-tree A -p P` records the raw tree `A`
and `worktree add --detach` materialises `A`, while a process inside that worktree reading
through Git sees `B` — `git show HEAD:f` returns the replacement and
`git status --porcelain` reports the untouched checkout modified. Two trees for one
snapshot is not a tree §4's "ground truth is the diff" could name, so upstroke removes the ambiguity
rather than detecting it. On the schema-4 path every child that runs Git over a snapshot — the
manager's own commands and read-only reads, and every gate, reviewer and implementer the runners
start, on the host runner and in a container alike — runs with `GIT_NO_REPLACE_OBJECTS=1`, which the
runners compose last. On Git 2.42 and later the variable is final, and no adapter, image or command
overlay can turn it back on. On Git 2.41, the oldest version `README.md` supports, it is not final: a
`core.useReplaceRefs = true` in a system, global, repository or worktree configuration file, or in
command-line configuration a child inherits, which is what a `GIT_CONFIG_*` pair in an image or an
overlay is, turns replacements back on for a child that carries only the variable (measured with Git
on 2.40.0 and 2.41.0 at each of those places; 2.42.0 and 2.43.0 held). The variable is also
process-wide, so a role that creates a repository of its own loses that repository's replacements
too. Both are filed against the schema-4 path, which no supported configuration activates
(`PR326-MANAGER-GIT-CHILDREN-CARRY-THE-VARIABLE-ALONE` for the manager,
`PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE` for the runners), and the v0.1 path
below has neither. A replacement graph is a ref outside the recorded inputs of a run, so a verdict
that depended on one would not be reproducible from the record; an operator who wants the replaced
history judged rewrites it, and the run judges what the repository then holds. The variable is one
constant, `NO_REPLACEMENT_OBJECTS` in `src/workspace_manager.rs`, named at each of those boundaries.

**Exact snapshots are §5's; the recorded graph is every run's.** The released v0.1 path has no exact
snapshot: its workspace and its ephemeral gate snapshots come from `src/workspace.rs`, frozen at
PR5. Every Git child of that module refuses replacements all the same, built by the one function
each of them starts from, which reads the same constant and also passes
`-c core.useReplaceRefs=false`: on Git 2.41 a configured `core.useReplaceRefs = true` outranks the
variable, and a command-line setting outranks every configuration file. The runner the v0.1
conductor installs keeps its gates, reviewers and implementers on the recorded graph of the
repository the run manages. For every role it appends two conditional includes to
`GIT_CONFIG_PARAMETERS`, after every entry the role inherits or its overlay sets —
`includeIf.gitdir:<common dir>.path` and `includeIf.gitdir:<common dir>/worktrees/*.path` — both
naming a file that holds `[core] useReplaceRefs = false`. Each component of those paths, `worktrees`
included, is matched with case unless the directory holding it finds it under the other case, and
then with either case of each ASCII letter; a letter outside ASCII is matched as the canonical path
spells it. Git reads `GIT_CONFIG_PARAMETERS` after every configuration file and after the counted
`GIT_CONFIG_*` pairs, so in the managed repository's main worktree and in every linked one, the
run's snapshots included, that setting comes last wherever Git names the Git directory in a
spelling those conditions match. A spelling the filesystem equates with the stored one that they do
not match, such as a letter outside ASCII in its other case or Unicode normalization, or a character
the filesystem folds onto an ASCII letter, reads the replaced graph, and a gate over an untouched
snapshot then fails
(`PR326-A-SPELLING-THE-FILESYSTEM-EQUATES-READS-REPLACEMENTS-IN-THE-MANAGED-REPOSITORY`, deferred:
which spellings name one directory is the filesystem's decision, not Git's and not this code's). A
repository a role creates for itself matches neither condition
and keeps its replacements. So does a repository whose path differs from the managed one's only in
the ASCII case of components held by directories that keep case, unless such a directory also holds
an alias of the managed path's component named with each ASCII letter's case swapped: a junction or
a bind mount then makes that directory look as though it folds, and the conditions reach the other
repository too (`PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING`, deferred). Nothing
is written into any Git configuration: the file lives in the private root, at
`<private root>/git/recorded-objects.gitconfig`, every run and resume writes it before its first
role starts, and the runner refuses to start a role while it is missing or altered. Configuration is
not an enforcement boundary: a later `git -c core.useReplaceRefs=true` inside a role still wins, and a
role that clears its environment loses the include. Git 2.40's `git merge-tree` reads a replaced
commit whatever its configuration says, so the floor is Git 2.41, and a run refuses an older Git by
name. Producer and consumer move together or not at all: a consumer of that workspace must read what
its own producer wrote or judge a tree nobody created, and a gate reading one graph over a snapshot
written from the other fails `git diff --exit-code HEAD` on a checkout nothing has
touched (measured on git 2.43: in both directions for a replaced tree or commit; for a replaced
blob, only over a snapshot written from the replaced graph). Until
`LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS` closed, the frozen module read the replaced graph and
the v0.1 runner was exempted to match it; the change that closed it amended the freeze for this
isolation alone, and the exemption's reason went with it.

Every transition is an event `{ts, event, task?, attempt?, rung?, profile?, data}` — including `question_raised`, `question_answered`, `design_defect`, `capacity_snapshot`, `pool_exhausted`, and `spend_down_engaged`. `status`, the ledger, and the capacity view are pure folds over this file.

**One fold, not two.** The engine never mutates run state directly: it appends an event and folds it back in through the same function `resume` and `status` use to rebuild state from the file, and it applies the event *as it will be read back* rather than as constructed. A live run and a replay of its own log are therefore the same computation, not two that agree by inspection. Two things deliberately do not survive replay — a session id and its `resume_next` flag, because both describe a conversation that believed it had left edits in a working tree that a crash has since rolled back (§14 pairs session-resume with tree retention precisely so the two never diverge).

`upstroke resume <run-id>` replays, verifies the run branch HEAD matches the last committed event (mismatch = refuse with an explanation), re-probes agents, re-snapshots capacity, and continues — parked questions intact. Git and the log cannot be updated atomically, so schema 3 makes the successful settlement itself carry the exact prepared identity: captured full run-branch ref, parent and tree feed hook-free `commit-tree`; the resulting commit, message, and deterministic private pin are verified before `attempt_finished` is appended. Publication compare-and-swaps the **recorded full branch ref**, never mutable symbolic `HEAD`, from the recorded parent to that commit, removes the pin with a non-dereferencing compare-and-swap, and then appends `task_committed`. Resume accepts only the resulting exact crash prefixes: parent plus matching pin means publish that object; commit plus matching pin means remove the pin; commit with the pin already gone means append the missing `task_committed`. A pin without a successful settlement is orphan residue and is removed without dereferencing symbolic refs. Any substituted or symbolic pin, third branch SHA, changed branch identity, or mismatched commit object refuses while preserving evidence. Schema-1/2 success has no prepared identity, so it is **never** adopted from parent plus subject alone; even a matching message can name an arbitrary tree. It also refuses when the frozen plan's digest moved, when the recorded chain structure no longer matches (a rung is an index into that chain), when the branch is gone, and when another process owns either the run or its physical worktree.

v0.2 extends that shipped exact-identity rule into two candidate/merge transactions. After fixing the verified tree, the engine creates and temporarily pins an immutable commit object; `candidate_prepared` is the sole successful settlement for that candidate-producing attempt and records exactly one complete attempt/base/commit/tree identity before the authoritative candidate ref moves, so resume adopts only that exact shape. Recovery then appends the missing `task_candidate_created`, whose append position establishes FIFO order. `merge_rejected` similarly embeds the complete frozen repair-task payload and admission state so rejection, task registration, key assignment, `AwaitingRepair`, and either runnable or human-gated repair state are one append rather than a rejection/spawn/question crash window. A human-gated admission's embedded question is itself authoritative for status, notification, and `upstroke answer`; it is not followed by a duplicate `question_raised`. Each `merge_verification_started` has exactly one terminal record: successful evidence lives inside `merge_prepared`, code failure inside `merge_rejected`, and infrastructure/crash outcomes inside unavailable/interrupted events. There is no standalone successful or failed finish event before the state-changing append. `merge_prepared` records disposition, expected integration SHA, proposed SHA, candidate, verification evidence, and repair lineage before `git update-ref` advances the run ref by compare-and-swap. On resume, expected-old means retry that same transition and append `task_merged`, proposed means append the missing `task_merged`, and any third SHA means refuse; `already_present` uses equal expected/proposed SHAs, so the same rule becomes a checked no-op. A proposed commit with no prepared/rejected terminal event is residue and is reverified; a dangling merge-review process is settled as interrupted with unknown spend first. The event schema moves rather than teaching `task_committed` a second meaning. The complete protocol and fault table are §26.

**Gates are taken from the record, not re-derived — and not refused over.** `run_started` records each effective gate in full (name, command, shell, timeout) and a resume rebuilds and runs *those*, exactly as it reads the review plan from the record rather than re-resolving who judges. This is the property a live run already has for free: config is parsed once at pre-flight and gates execute from memory, so a mid-run edit to `upstroke.toml` cannot change what a running task is verified against. Honouring the same snapshot across an interruption is what makes every `task_committed` in one log mean the same thing — and it matters concretely once runs self-host, because the workspace an implementer edits *contains the `upstroke.toml` its own gates come from*. Refusing on a mismatch was the first design and was worse in both directions: it left the weakened-gate case detected but the run dead, and it made a gate edit that the run's own reviewed task legitimately committed permanently unresumable. A config that differs today is a warning naming the difference, not an error; the edit simply applies to the next run. Logs predating the record re-derive and warn, saying whether the recorded gate *names* still match — which is proof rather than suspicion when they do not — and that resume writes what it settled on into its own `run_resumed`, so the next one is an ordinary record-bearing resume rather than a second re-derivation that could land somewhere else. `shell` is recorded because it is half of what a command means (`cmd = "true"` always passes under `sh` and is not a program at all under `cmd.exe`); the portability that argued against pinning it does not exist anyway, since `private_dir` already records an absolute host path. The finding and the withdrawn refusal remedy were recorded on 2026-08-11. An attempt the log ends mid-flight is settled as `attempt_interrupted`: recorded in the ledger with unknown spend, but not counted against the rung's allowance, because nothing judged the code — the same rule §19 applies to an outage.

**Effort and worker bindings are taken from the same run snapshot.** `run_started.effort_policy` records the resolved implementation value at small, mid, and frontier plus the review value, while every chain records each rung's exact agent and model plus whether it was pinned. Every worker and every review pass reads those snapshots, so editing `[routing.effort]`, adding a pin, or installing another CLI between processes cannot change one run's execution identity or standard. A mismatch warns and continues with the record; a changed chain shape refuses because recorded rung indices would no longer mean the same thing. Start a new run to adopt current routing.

Those identity fields require event schema 2. A schema-1 log remains readable by a current binary: its first resume re-derives the missing policy and bindings once with explicit warnings, then records them on `run_resumed`. Before it appends any event whose meaning depends on the new identity, it appends `run_schema_upgraded { from: 1, to: 2 }`. Current replay validates that transition; an old binary does not know the marker and therefore refuses instead of silently continuing a run whose new fields it would ignore. Later resumes are record-bound and do not append a second marker.

The complete-review and atomic sequential-settlement contracts begin at event
schema 3. A schema-2 binary ignores the recorded per-pass timeout and retains
its 60 KiB prompt truncation; it would also ignore the ladder transition now
embedded in a failed `attempt_finished` and could spend the same known failure
again after a crash. Fresh runs therefore write schema 3, and a current binary
resuming a schema-1 or schema-2 run appends a transition to 3 before another
attempt. Older binaries refuse that opening schema or transition instead of
silently applying weaker verification or replay semantics. Every failed
sequential attempt embeds its retry, escalation, deferral, terminal failure, or
parking decision in the same durable settlement. A parking settlement carries
the authoritative question too; it is not followed by separate ladder,
`question_raised`, or `task_parked` events that a crash could strand between.
A declined `question_answered` likewise freezes the contemporaneous
`on_task_failure` decision, so resume can append a missing task settlement
without reinterpreting the human's already-durable answer through edited config.

Schema-3 success validation uses `AttemptRecord::is_successful`: no failure record and every
review pass approved. A non-passing review without a failure record is inconsistent and is
refused before replay. It cannot authorize a prepared commit or the following `task_committed`.
Normal review failures retain their recorded failure and ladder or parking decision. This
checks the existing settlement contract without adding fields or changing the schema number.

The v0.2 execution topology consequently begins at event schema 4 because its
task states and transactions change execution meaning. Fresh topology runs write
schema 4 in `run_started`; older binaries reject them before folding. Existing
schema-1 through schema-3 runs finish through the sequential path, including the
review-contract upgrade when needed. No in-flight run appends a 3 → 4 upgrade:
starting a new run is the compatibility boundary for adopting worktrees,
candidates, and the merge queue.
