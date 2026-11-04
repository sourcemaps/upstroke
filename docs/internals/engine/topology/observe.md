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

`TopologyReport::render` (which names the runner identity), then the
accounting projection's warnings and its ledger (`RunReport::render_ledger`,
read through `accounting` below), then the lines legacy `status` adds: the log's
schema and committed line count, the run's state (running now; or interrupted,
with the resume command), a torn tail's byte count, the open questions with the
command that answers each, and where the transcripts are.

The ledger is L3 (regression review 3, finding 3; DESIGN §18, "`status` prints
the report and its ledger (§13)"). Until L3 a schema-4 `status` printed only the
topology report: task states and the integration ledger, but no spend. The
worker's known `$0.0100`, the run's total, the task ledger and the pool drain
that `run` had printed could not be recovered from `status`. The only cost on
the screen was the fast integration row's `cost ? (unknown)`. Both new parts
come from the existing renderings, `RunReport`'s warning lines and
`render_ledger`. So the floors J3, J12, K4 and K5 put in the projection read the
same in `status` as in the run's own report: an unpriced pass, an unattributed
or interrupted integration verification, and an attempt killed in flight.

## `impl Observed` › `fn accounting(&self) -> RunReport {`

`RunReport::from_topology`, the projection `run` and `resume` return, over this
log. A finished run and one another process holds are projected as committed.

**An interrupted run is projected as its resume will record it.** Nothing holds
it and it has not finished, so whatever its log has in flight (an attempt
started and never settled, or an integration verification started and never
ended) was cut off with its spend unknown. The legacy `status` settles such an
attempt in memory for the same reason (`status::load`'s `settle_interrupted`).
Here the committed events are followed, in an owned copy, by the interrupted
settlements `engine::topology::closure::in_flight` names: the same identities
recovery's steps (d) and (f) will settle. The projection then reads the cut-off
attempt as counted with nothing known, and the verified task's review as a floor.
The total and the pool drain carry `?`, and the legend says what it means.
Without this, the total of a run killed mid-attempt read as exact.

- The settlement bodies are closure's own (`InFlight::interrupted`). Their
  details are written for a halt, and the ledger prints none of them.
- Nothing is written, and the fold is not replayed again. The topology report
  above the ledger still shows the generation in flight and the verification
  `open`, as the log does.
- A run another process holds is not settled: its attempt is working, not
  interrupted (`status.md` says the same of the legacy view).

## `impl Observed` › `pub fn describe(&self, event: &TopologyEvent) -> String {`

One line per event for `--follow`: its timestamp, its kind, and for the
attempt, question, merge and finish events the task (by display id) and the
detail an operator watches for.

**The assembled line goes through `util::terminal::one_line`** (L1, regression
review 3, finding 1; DESIGN §18), the sanitization the settled rendering
(`TerminalLines`) and the legacy `status::describe` use: a newline, carriage
return or tab in any field becomes a space and every other control character
its visible escape, so one event is one physical line and no field reaches the
terminal as a control. Every field on the line is on-disk data the line does not
choose: a task's display id comes from the plan, an agent and model from the
recorded binding, a sha, question id, run id or incarnation from the record, and
the timestamp from the record too. Until L1 the line was printed as assembled: a
task annotated `id=t1` followed by ESC `[2J` made `status --follow` clear the
operator's screen twice, on its `attempt_started` and `task_merged` lines (the
reviewer's executed witness), while `status` printed the same id escaped.

An `attempt_started` line is the attempt's admission. Its slot permit is
acquired after it, so two attempts both started are not two workers running.

## `pub fn open_question_candidates(repo_root: &Path, runs: &[String], wanted: &str) -> OpenCandidates {`

D3: what the schema-4 runs among the caller's runs hold for a typed id, as
structure rather than as a verdict or a sentence (root's correction
PR12-ANSWER-CROSS-SCHEMA-PREFIX, 7021b42d): the open question whose full id it
is; the open questions whose id begins with it; the questions a log records as
raised and no longer open whose id is it or begins with it; and the runs whose
log could not be observed, carried rather than dropped. ASCII
case-insensitive, as the legacy finder is. A legacy log observes as `None` and
contributes nothing here. There is no question-payload file: root withdrew
that option.

**Nothing here decides.** `answer::decide` weighs these candidates together
with the legacy question files of the same runs, so neither finder's lone match
stands for the other's. The runs come from the caller: `answer` passes the
committed directories of its one census, so both finders walk the same set.

**A settled question stays a candidate for a prefix**, as a legacy question's
file stays one once it is answered: a prefix it shares with an open question of
either schema is ambiguous rather than that open question's, so an id typed
for a question that has since settled cannot land an answer on another.

## `pub fn check_reply(asked: &OpenAsked, answer: &crate::ir::Answer) -> Result<(), UpstrokeError> {`

The validations the schema-4 loop adds to today's. A question that binds an
agent (`QuestionOrigin::Admission` with a frozen binding) accepts only one of
its options, because the loop activates a binding only through an option the
spawn froze (`run.rs`, `answer_for`). An `ApproveSpend` question does too,
because a spend is approved only by choosing its option, never by words
(J1). Refusing here keeps those refusals out of the run.

## `pub(super) fn question_opened_by(event: &TopologyEvent) -> Option<&FrozenQuestion> {`

The question an event opened, whichever kind opened it.
`open_question_candidates` reads it to name the questions a run has
already settled, and since J2 `run::Brief::replay` reads it too, to judge
an answer's words against the options of the question it answered.

## `pub fn follow(`

Each poll is a fresh observation, and the events past the count already
printed are described; it stops when the run has finished and nothing holds
it, or after the idle budget when nothing holds an unfinished one, as legacy
`follow` does. It writes nothing itself: each line goes to the caller's
`emit`, because a topology module may not write to a stream
(`std::io::Write::write_fmt` is a disallowed method here), and
`status::follow` is what prints them.
