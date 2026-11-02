# `src/engine/topology/observe.rs`

Extended notes for [`src/engine/topology/observe.rs`](../../../../src/engine/topology/observe.rs).

The source defines behavior; these notes hold the module's contracts and rationale.
Each code span in a section heading is an exact source fragment. Search it as a fixed string
in the linked module, using the enclosing item to distinguish repeated lines.

## Module

The read-only observed prefix of a schema-4 log (PR12, D1 as root settled
it): what `upstroke status` and `status --follow`, `upstroke answer` and the
question it shows, and `upstroke export-decisions` read when a run's log is
schema 4.

**Complete lines only, a stable reread, a checked replay.** The reader takes
the bytes up to the last commit marker (`\n`) and nothing after it; it reads
the log twice and accepts the second read only when it begins with the first
read's committed bytes (a log grows only at its end), with the run's liveness
the same before and after; and it parses the committed lines with
`TopologyFold::parse_log` and replays them with `TopologyFold::replay` over the
frozen plan, whose digest is checked against the record exactly as a resume
checks it (`entry::frozen_inputs`).

**It never truncates, syncs or takes a run's lock, and its fold authorizes no
effect.** A torn tail is reported, not removed: the next write command on the
run settles it through the barrier. The fold this module builds is a
projection for reading; `events::log::establish_stable_prefix`'s
`StablePrefix` stays the only route from a log to an effect.
`events::log::tests::the_stable_prefix_barrier_is_the_only_way_a_log_becomes_a_topology_fold`
counts this module as the second, and only other, production place that
builds a fold from a log, once each, and asserts that it names no effect,
lock or barrier.

**Liveness is `rundir::is_running`**, the probe legacy `status` uses: it asks
who holds the run lock without taking it, and only when nobody does it
momentarily tries the cleanup lease, creating and changing nothing.

**Root's Q6, answered.** Root's D1 asked implementation to confirm that no
census restricts production callers of `TopologyFold::replay` or
`TopologyFold::parse_log`. One does: the census named above held them to the
barrier alone. It is amended, not removed: the observer is admitted by path
and by count, and constrained by what it may not name.

## `pub fn is_topology_log(bytes: &[u8], log: &Path) -> Result<bool, UpstrokeError> {`

The header probe of `entry::route_resume`, for readers: schema 4 reads here,
a legacy schema or a header that cannot be read goes to the legacy reader
(which diagnoses it), and a schema above the ceiling refuses with the probe's
own refusal.

## `pub fn observe(repo_root: &Path, run_id: &str) -> Result<Option<Observed>, UpstrokeError> {`

`None` for a legacy log, so a legacy run's reader is unchanged; the probe read
is separate from the stable read, so a legacy log is never subjected to the
schema-4 reader's rereads.

## `fn stable_read(log: &Path, public: &Path) -> Result<(Vec<u8>, bool), UpstrokeError> {`

Eight attempts, as legacy `status` allows its own: two reads and two liveness
probes per attempt. A rewrite under the committed bytes, or a run that changed
hands between the probes, retries; one that keeps doing so refuses rather than
reporting a prefix it could not hold still.

## `impl Observed` › `pub fn render(&self) -> Result<String, UpstrokeError> {`

`TopologyReport::render` (which names the runner identity), then the lines
legacy `status` adds: the log's schema and committed line count, the run's
state (running now; or interrupted, with the resume command), a torn tail's
byte count, the open questions with the command that answers each, and where
the transcripts are.

## `impl Observed` › `pub fn describe(&self, event: &TopologyEvent) -> String {`

One line per event for `--follow`: its timestamp, its kind, and for the
attempt, question, merge and finish events the task (by display id) and the
detail an operator watches for.

## `pub fn find_open_question(repo_root: &Path, wanted: &str) -> QuestionLookup {`

D3: the open questions of every schema-4 run, from the fold, by full id or
unambiguous prefix, ASCII case-insensitive as the legacy finder is. A run
whose log cannot be observed is carried as unread rather than dropped, so a
lone prefix match is unambiguous only where every run was read; a full id
still answers. A question the log records as raised and no longer open is
`Closed`, so `answer` can say so instead of "not found". There is no
question-payload file: root withdrew that option.

## `pub fn check_reply(asked: &OpenAsked, answer: &crate::ir::Answer) -> Result<(), UpstrokeError> {`

The one validation the schema-4 loop adds to today's: a question that binds an
agent (`QuestionOrigin::Admission` with a frozen binding) accepts only one of
its options, because the loop activates a binding only through an option the
spawn froze (`run.rs`, `answer_for`). Refusing here keeps that refusal out of
the run.

## `pub fn follow(`

Each poll is a fresh observation, and the events past the count already
printed are described; it stops when the run has finished and nothing holds
it, or after the idle budget when nothing holds an unfinished one, as legacy
`follow` does. It writes nothing itself: each line goes to the caller's
`emit`, because a topology module may not write to a stream
(`std::io::Write::write_fmt` is a disallowed method here), and
`status::follow` is what prints them.
