# `src/engine/topology/coordinator/tests/snapshot_registration.rs`

Extended notes for [`src/engine/topology/coordinator/tests/snapshot_registration.rs`](../../../../../../src/engine/topology/coordinator/tests/snapshot_registration.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## `struct FEffects {`

F's coordinator-thread effect hooks: forward every phase to the harness, run an act once at a site's phase, arm an act before the next registry access of this thread (`fixture::before_registry_access`) at a site's phase, or inject one error at a site's phase.

## `struct FHooks {`

F's coordinator `TopologyHooks`: the harness's hooks with `FEffects` for the effects and one-shot acts after a matching event is folded.

## `fn f_only_checkout_under(namespace: &std::path::Path) -> PathBuf {`

The one checkout under a slot namespace (`tasks/`, `merge/`), whichever incarnation's instance it is.

## `fn f_prune_whole(base: &std::path::Path, checkout: &std::path::Path) {`

A prune no engine process started, deleting the whole entry of a checkout: its `.git` set aside, `git worktree prune --expire=now` from the base, the `.git` put back (#329's d9 construction).

## `fn f_cls_tasks() -> [WideTask; 3] {`

T-C1-CLS-RUN's tasks: beta deletes every tracked file, so its fast merge leaves an integration head whose tree is empty; gamma, dispatched at the base, modifies `a.txt`, so its stale pick stops on a modify/delete conflict.

## `fn f_stale_tasks() -> [WideTask; 3] {`

Three independent tasks whose third, gamma, is integrated stale behind beta.

## `fn f_step_to_error(`

The width-1 `step` loop until the run finishes or a step errs.

## `fn f_gamma_queued(tag: &str, tasks: &[WideTask], runner: RecordingRunner) -> Wide {`

Width 1 cannot make a stale candidate (selection integrates before it dispatches), so: a durable width-3 run whose first command ends at gamma's pick (an injected error there leaves nothing durable: the pick is classified unclassified and the staging reclaimed), resumed so that gamma's stale integration is the resumed run's next step.

## `fn f_finish(mut wide: Wide, incarnation: &str, tasks: &[WideTask]) -> Wide {`

Resume in a new incarnation and drive the run to its end.

## `fn f_delete_staging_index_at_the_classification(`

Arms, at the proposal pick's `Before` phase, an act for the next registry access of the coordinator's thread: the classification's revalidation, since the pick's primitive makes none. The act deletes the staging entry's `index` (and, for RESIDUAL (iii), writes it again with `git read-tree HEAD`).

## `fn f_cls_run_width_three(tag: &'static str, concurrency: bool) {`

T-C1-CLS-RUN at width 3, and T-CONC-5 with `concurrency`: at C, gamma's conflict reads as an empty pick and is published already present at the unchanged head; under F the command ends `RegistryRefused` with nothing appended for gamma's sequence, the live attempt's completion discarded (T-CONC-5), and the next resume integrates gamma again and rejects the conflict.

## `fn a_stale_conflict_read_through_a_deleted_staging_index_ends_the_command_at_width_three() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-CLS-RUN, width 3 (C1-CLS, FUF-R1-1).

## `fn a_classification_refused_beside_a_live_attempt_cancels_it_and_the_resume_integrates_again() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-CONC-5.

## `fn a_stale_conflict_read_through_a_deleted_staging_index_ends_the_step_at_width_one() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-CLS-RUN, width 1: the refusal leaves through `TopologyRun::integrate`.

## `fn a_staging_index_written_again_before_the_classification_reads_as_already_present() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-RESIDUALS (iii), pinned: an index written again before the classification's check reads whole, so the conflict is published already present, at C and under F (R-REWRITE, X-1 item 1).

## `fn f_prune_staging_when_gamma_verification_starts(`

Prunes the staging entry whole once gamma's `merge_verification_started` is folded: after the staging add, before the verification's diff.

## `fn f_interrupted_and_verified_again(resumed: &Wide, refused: SequenceId) {`

The next resume settles the refused verification interrupted and verifies gamma under a new sequence, spending no deferral.

## `fn a_verification_whose_staging_entry_is_pruned_before_its_diff_ends_resumably_at_width_three()`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P1-VERIFY, width 3: at C the diff's Git error is an outage and a deferral is spent; under F nothing follows `merge_verification_started`.

## `fn a_verification_whose_staging_entry_is_pruned_before_its_diff_ends_resumably_at_width_one() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P1-VERIFY, width 1.

## `struct FPolicy {`

A test `ReviewInputPolicy`: acts on a checkout of one namespace (`merge` for a verification, `tasks` for an attempt) — once, or on every call — and then delegates to the legacy policy; it counts its calls.

## `fn f_removing(name: &'static str) -> FCheckoutAct {`

An act removing one name of a checkout's entry.

## `fn f_pruning(base: PathBuf) -> FCheckoutAct {`

An act pruning a checkout's entry whole.

## `fn f_corrupting() -> FCheckoutAct {`

An act writing a corrupt `index` (a regular file, so the check reads whole).

## `fn f_unstaging(and_delete_the_index: bool) -> FCheckoutAct {`

An act writing a genuine unstaged change, and optionally deleting the `index`.

## `fn f_keep_the_store(wide: &Wide) {`

A linked checkout outside the execution root that keeps the registration store when a slot's entry is pruned.

## `fn f_policy_cases(width: usize) -> Vec<(String, Result<(), String>)> {`

T-C1-4's cases at one width: pruned whole with the store kept, pruned whole with the store gone, `HEAD` removed, `commondir` removed. At C the policy's Git error is an outage (a deferral spent); under F a registry refusal.

## `fn f_report(cases: &[(String, Result<(), String>)]) {`

Collects every case's outcome before asserting, so a red run reports every case's state.

## `fn a_review_input_read_failing_in_a_staging_checkout_whose_registration_is_not_whole_is_refused_at_width_three()`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-4, width 3.

## `fn a_review_input_read_failing_in_a_staging_checkout_whose_registration_is_not_whole_is_refused_at_width_one()`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-4, width 1.

## `fn a_review_input_git_error_in_a_whole_staging_registration_settles_an_outage_at_both_widths() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-4's control: a corrupt index in a whole registration is the outage it was, at C and under F (option A, not A′).

## `fn a_review_input_problem_read_through_a_deleted_staging_index_is_refused_not_hidden() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-4-ANS: with the staging `index` deleted the legacy policy answers no problem (E8); at C the verification is judged and published; under F the read is refused.

## `fn a_review_input_problem_in_a_whole_staging_registration_is_a_review_input_failure() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-4-ANS's control, at C and under F.

## `fn f_attempt_run(`

One task's run at width 1 (`step`, the caller's hooks) or 3 (`run_concurrently`, pipeline hooks from `f_pipeline_hooks`), with a test policy and an optional act at a site's phase.

## `struct FPipelineEffects {`

Pipeline-thread effect hooks acting at one site's phase: a pipeline's capture runs on its own thread with hooks from the factory.

## `struct FPipelineHooks {`

The pipeline-thread `TopologyHooks` over `FPipelineEffects`.

## `fn f_pipeline_hooks(`

The `HooksFactory` producing `FPipelineHooks`.

## `fn f_once_in_the_task_slot(root: PathBuf, name: &'static str) -> FSharedAct {`

An act, once, removing one name from the only task slot's entry.

## `fn an_attempts_review_input_problem_read_through_a_deleted_index_is_refused_not_hidden() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-AP, widths 1 and 3: at C the attempt is judged without its review-input failure; under F the read is refused.

## `fn an_attempts_review_input_problem_in_a_whole_registration_fails_the_attempt() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-AP's control, at C and under F.

## `fn f_no_change_responder() -> crate::engine::topology::scaffold::Responder {`

A worker that completes and changes nothing (an empty diff).

## `fn an_attempts_failure_assessed_in_a_slot_whose_registration_is_not_whole_is_refused_not_spent()`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P3 (a) and (b), widths 1 and 3: a failed worker, or one with no change, the slot's `gitdir` or `index` removed at the `After` phase of `Object.CandidateWriteTree`, so that no check meets the deletion before placement 3 (the policy is bypassed and never called). At C the attempt is spent; under F placement 3 refuses. The engine suite has no seam after the assessment, so the deletion lands after the capture's last check, where nothing reads the index before placement 3.

## `fn an_attempts_failure_assessed_in_a_whole_registration_is_spent() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P3's control, at C and under F.

## `struct FReview {`

A `ReviewPasses` that runs an act at one call and delegates to the legacy passes.

## `fn a_promotions_path_read_through_a_deleted_index_ends_the_step_without_a_candidate() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-ANS-PROMO, width 1: the review pass deletes the attempt slot's `index` after its own snapshot was taken; at C the promotion records the base's paths as deletions; under F the path read is refused and no candidate is prepared.

## `fn f_prune_own_entry(checkout: &std::path::Path) {`

Prunes a checkout's entry whole, running `git worktree prune` inside the common git dir the entry lives in.

## `enum FSubject {`

Attempt or verification: which judge a placement-2 test drives.

## `fn f_gate_responder(`

A runner responder whose first gate of the subject runs a stub gate in its snapshot and exits with the gate's code; everything else is the scaffold's.

## `fn f_construct(name: Option<&'static str>, snapshot: &std::path::Path) {`

Removes one name of a snapshot's entry, or prunes it whole.

## `fn f_rev_parse_gate(name: Option<&'static str>) -> FGate {`

A stub gate: the construction, then `git rev-parse --verify HEAD` in the snapshot.

## `fn f_index_gate() -> FGate {`

A stub gate reading the index: `index` removed, then `git ls-files --error-unmatch a.txt`.

## `fn f_forbidden_gate(delete_the_index: bool) -> FGate {`

T-C1-P2-PASS's gate: fails while `*.forbidden` is tracked; with the `index` deleted it reads an empty index and passes.

## `fn f_blind_gate_in_a_pruned_snapshot() -> FGate {`

A gate that reads no Git state, passing in a snapshot pruned whole: the liveness cost of checking every verdict.

## `enum FAnswer {`

What a stub review pass answers.

## `struct FReviewStub {`

A `ReviewPasses` that, at one pass of the subject, prunes its own snapshot's entry and answers unavailable, a pass, or a pass with a process count that disagrees.

## `fn f_drive_subject(`

Drives the four of `F_MATRIX`: an attempt at width 1 and 3, a verification at width 1 (via `f_gamma_queued`'s construction) and 3.

## `fn f_refused_case(`

A case's registry refusal naming the given words, or why not.

## `fn f_bounded_case(`

One case of the three not-whole matrices below under `bounded()`: its setup, its run, the read of its outcome and the drop of its run all happen inside one `BOUND`, named by the case, and its outcome comes back for `f_report`. The bound is the case's because the cases are independent and run one after another: one `BOUND` around a whole 12- or 16-case matrix measured how many cases a test chains and how loaded the runner was, not whether any one case was wedged, and it fired on the Windows guest's full suite while every case was progressing (`G6-F-MATRIX-AGGREGATE-BOUND`). A case that never ends still fails its test within `BOUND`, naming the case; the cases stay sequential, and `f_report` still reads every outcome.

## `fn a_gate_verdict_read_in_a_snapshot_whose_registration_is_not_whole_is_refused() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-GATE: pruned whole, `HEAD`, `commondir`, and the index gate's `index`. At C a verification is rejected and an attempt spent; under F placement 2 refuses. Each of its 16 cases runs under its own `BOUND` (`f_bounded_case`).

## `fn a_genuinely_failing_gate_in_a_whole_snapshot_is_judged() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-GATE's control, at C and under F.

## `fn a_gate_that_passes_through_a_deleted_index_is_refused_not_published() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-PASS: at C the gate passes through the empty index and the candidate is published or promoted; under F refused.

## `fn a_gate_reading_a_whole_index_fails_on_the_forbidden_file() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-PASS's control, at C and under F.

## `fn a_passing_gate_that_reads_no_git_state_in_a_pruned_snapshot_is_refused() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-PASS's liveness control: a genuine pass in a pruned snapshot is refused under F (§3.4's cost, pinned).

## `fn f_gate_of(subject: FSubject, invocation: &InvocationId) -> bool {`

Whether an invocation is one of the subject's gates: an attempt's, or a verification's.

## `enum FLaunch {`

How a gate run fails, or does not, in F-R1's constructions: through the production container runner and Git-view reader over a fake container runtime (`HEAD` removed, the entry pruned whole, intact, the runtime's create failing), or as a stub Runner error after `HEAD` is removed (lost after it started, cancelled, unresolved).

## `struct FLaunched {`

One gate launch as the responder saw it: whether it failed, with its fate and whether it was a cancellation; its error; the snapshot's `HEAD`, and whether it was there after the launch.

## `fn f_launch(`

Runs one gate as the construction says. The container cases build the production `ContainerRunner` with `RoleGitView` over `container_host()`, in a context fixture of their own (an underscored incarnation and repo key, which `ContainerName` accepts), and make the deletion immediately before the launch, as the review's witness did; the stub cases remove `HEAD` and answer the Runner error.

## `fn f_launching_gate_responder(`

A runner responder sending every gate of the subject through `f_launch` and recording each launch; everything else is the scaffold's.

## `fn f_first_launch(launched: &FLaunches) -> Result<FLaunched, String> {`

The first recorded launch, or why there is none.

## `fn f_names_path(message: &str, path: &std::path::Path) -> bool {`

Whether an error names a path, in either separator spelling: both are spelled with `/` before the comparison. The container's Git view rebuilds the path it reads from its components, so on Windows its error spells the snapshot's `HEAD` with `\` throughout, while the test's `head` keeps the `/` of the checkout's `.git` pointer as Git wrote it, joined to `HEAD` with `\`; #333's `test (winguest)` at `629cdef5` failed F-R1's eight container cases on that difference. On Unix a `\` is a file-name character that this reads as a separator; no fixture path holds one.

## `fn f_launch_witnessed(launched: &FLaunches, launch: FLaunch) -> Result<(), String> {`

The construction's premise: the first launch failed with the fate it makes (`NeverStarted` from the container's view, `Gone` from the stub) and not as a cancellation, the container's error names the deleted `HEAD` in either separator spelling (`f_names_path`), and `HEAD` was still absent after it.

## `const F_WINGUEST_LAUNCHES: [(&str, FLaunch, &str, &str); 8] = [`

The eight container launches of F-R1's test on the winguest at `629cdef5` (#333's CI run 37393050473, job `test (winguest)`), each error and `head` exactly as that cell's premise failure printed them.

## `const F_LINUX_LAUNCHES: [(&str, FLaunch, &str, &str); 2] = [`

Two container launches of the same test on Linux (an attempt at width 1, `HEAD` removed and the entry pruned whole), exactly as a run printed them, in the one spelling Linux has.

## `const F_LINUX_LOST_LAUNCH: (&str, &str) = (`

The stub's lost run from that Linux run: its error names no path.

## `fn f_premise_of(`

`f_launch_witnessed` over one recorded launch.

## `fn the_launch_premise_names_the_deleted_head_in_either_separator_spelling_and_nothing_else() {`

F-R1's premise, checked on exact strings: it accepts each recorded Windows and Linux launch as printed (the raw comparison of `629cdef5` finds the head in the Linux errors only), and refuses, in both spellings, another snapshot's `HEAD`, another entry's `HEAD` in the same store, the entry's `gitdir` named instead of `HEAD`, the entry directory alone, an error cut before its path and an empty one, `HEAD` there after the launch, a cancellation, no failure, the stub's `Gone` under the container's construction and the container's `NeverStarted` under the `Lost` one. The `Lost` construction, whose stub error names no path, is checked apart. It reads strings only, so it judges the Windows spelling on every platform; what the eight Windows cases end with is for the test itself to show there.

## `fn a_gate_run_failing_in_a_snapshot_whose_registration_is_not_whole_is_refused() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` — the regression test of the review's F-R1 (#333 at `157c9cab`), placement 2 at a gate's failed run, at `F_MATRIX`'s four cells: the container gate's launch with `HEAD` removed, and with the entry pruned whole, just before it; and a stub run lost after it started. At `157c9cab` a verification settles `merge_verification_unavailable`, and, repeated, parks (`Finished { outcome: Parked }`), and an attempt ends with the Runner's error; under the repair each is the registry refusal naming the gate's run. Each of its 12 cases runs under its own `BOUND` (`f_bounded_case`).

## `fn a_container_gate_launched_in_a_whole_snapshot_is_judged() {`

F-R1's control: the same production container runner and Git-view reader over an intact registration materialize every gate's view, and the run completes with no outage, at `157c9cab` and under the repair.

## `fn a_gate_launch_failing_in_a_whole_snapshot_is_the_outage_it_was() {`

F-R1's control: a container gate whose create the runtime refuses in a whole snapshot (`NeverStarted`) settles a verification's outage as it did, and ends an attempt's command with the Runner's error, at `157c9cab` and under the repair (option A, not A′).

## `fn a_cancelled_or_unresolved_gate_run_ends_the_command_as_it_did_whatever_the_registration() {`

F-R1's control: with `HEAD` removed, a cancelled gate run ends the command with the Runner's cancellation at every cell, and an unresolved one with the Runner's error at width 1 and with the coordinator's refusal for the unresolved process at width 3, at `157c9cab` and under the repair: the check reads neither.

## `fn a_review_verdict_read_in_a_snapshot_whose_registration_is_not_whole_is_refused() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-REVIEW: pass 0 unavailable, pass 1 unavailable after pass 0 passed, and pass 0 passing, each after its snapshot's entry was pruned. Under F refused after the pass's charge. Each of its 12 cases runs under its own `BOUND` (`f_bounded_case`).

## `fn a_review_verdict_read_in_a_whole_snapshot_is_judged() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-REVIEW's control, at C and under F.

## `fn a_review_whose_process_count_disagrees_keeps_that_refusal_when_its_snapshot_is_not_whole() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-P2-ORDER: the process-count refusal precedes placement 2, at C and under F.

## `fn a_review_input_read_over_a_lost_shared_index_settles_an_outage() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-RESIDUALS (i), pinned: a split index's shared file removed before the policy read; the check reads whole and the outage settles, at C and under F (R-OUTSIDE, X-1 item 3).

## `fn a_gate_index_written_again_before_the_check_reads_whole_and_is_judged() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C1-RESIDUALS (ii), pinned: a gate's snapshot `index` removed and written again by `git read-tree HEAD` before the check; the genuine failure is judged, at C and under F (R-REWRITE).

## `fn f_files(root: &std::path::Path) -> BTreeMap<PathBuf, Vec<u8>> {`

Every file under a directory with its bytes, for byte comparison of a kept slot.

## `fn f_retained_of_alpha(body: &TopologyEventBody) -> bool {`

The first task's retained `attempt_finished`.

## `fn f_retained_retry(`

One task whose first attempt's gate fails, so its generation is retained; at that fold an act changes the slot; then the retry. Widths 1 and 3.

## `fn a_retained_retry_over_a_pruned_populated_slot_refuses_and_keeps_the_generation_retained() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C2-RETAINED, widths 1 and 3: under F the retry refuses, the `Retry` reservation is cancelled (no entitlement held), nothing is appended, the generation stays `RetainedIdle` and the slot is kept byte for byte; at C the generation is closed and the slot scrubbed.

## `fn f_closed_worktree_missing_of_alpha(events: &[TopologyEvent]) -> bool {`

A `generation_closed{WorktreeMissing}` of the first task.

## `fn a_retained_retry_over_a_whole_slot_that_no_longer_holds_its_tree_closes_and_scrubs() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C2-RETAINED's control: a genuine tree mismatch in a whole registration closes and scrubs, at C and under F.

## `fn a_retained_retry_reading_residue_closes_and_scrubs_whatever_the_registration() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C2-RETAINED-RESIDUE: a held `index.lock` and a lost `index` read as residue, closed and scrubbed at C and under F — the limit the K-texts and P-6′ state.

## `fn f_open_generation_left(tag: &str) -> Wide {`

A durable run whose dispatch's add fails at its `After` phase: `task_dispatched` is durable and no attempt started (an `OpenNoAttempt` generation).

## `fn an_open_generation_continued_live_over_a_pruned_populated_slot_is_kept_and_refused() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C2-OPEN, live: after recovery's step (g) recreated the open generation, its slot is pruned and the live continuation refuses, appending nothing; the generation stays `OpenNoAttempt` and the slot is kept.

## `fn a_resume_over_an_open_generation_whose_populated_slot_is_pruned_refuses_before_run_resumed()`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C2-OPEN at recovery step (g): the dead incarnation's populated instance, pruned, is kept, and the resume refuses before `run_resumed` (`try_resume_over` hands the run back).

## `fn f_run_finished(body: &TopologyEventBody) -> bool {`

The run's `run_finished`.

## `fn f_converges_with_the_store_gone(tag: &str, halted: bool) {`

T-C3-2: at the fold of `run_finished` a slot still populated is left with its entry pruned, the store's last, so the store is gone; at C the finalization's removal refuses (ST-18); under F closure 3 removes the checkout and the execution root goes.

## `fn a_complete_runs_finalization_converges_over_a_populated_slot_with_the_store_gone() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C3-2, a `Complete` run.

## `fn a_halted_runs_finalization_converges_over_a_populated_slot_with_the_store_gone() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-C3-2, a `Halted` run.

## `fn a_verification_refused_at_placement_two_discards_a_waiting_attempt_and_the_resume_verifies_again()`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-CONC-1: the attempt held until gamma merges is live when gamma's verification refuses at placement 2; its completion is discarded, and the next resume settles it interrupted and verifies gamma again.

## `type FSignal = Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>;`

A one-way signal between the coordinator's hooks and a pipeline thread, waited on within `BOUND`.

## `fn f_beta_refusing_after(blocked: FSignal, release: FSignal) -> FPolicy {`

Beta's policy act: signal that it is blocked, wait for the release, then write an unstaged change and delete the `index`, so C1-AP refuses.

## `fn f_only_beta(policy: FPolicy) -> FPolicy {`

Applies a policy's act to the second task's slot only, on every call.

## `struct FAfterBetaBlocks {`

T-CONC-2's runner: alpha's worker waits, within `BOUND`, until beta is blocked in its deletion policy, then runs on the scaffold's runner; every other invocation runs at once. So alpha's question, and the halt it brings, follow beta's entry into its policy by construction: no schedule runs alpha first, and one forced fails loudly.

## `struct FHanded {`

What beta's policy handed back to the engine: whether the unstaged change was written and the `index` gone, whether the answer is one the check after it refuses while the registration is not whole (an answer, or a Git error), and what that registration check found.

## `struct FBetaRead {`

T-CONC-2's policy: beta's, then a record of the checkout it hands back, read with the production registration check (`dispatch::registration_whole`) as the policy returns; C1-AP runs that check next, on the same thread and over the same checkout.

## `struct FInputsOf {`

`AttemptPlans` counting one task's review-input requests, and delegating: a pipeline whose review-input read is refused returns before it asks for its inputs.

## `fn a_refusal_arriving_after_a_halt_is_discarded_unread_and_the_halt_scrubs_its_slot() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-CONC-2: alpha's declined question halts the run while beta waits in its policy; beta is released when `question_answered` is folded, damages its checkout, is refused, and its completion is discarded unread (counted, named by no warning); the halt's closure scrubs the in-flight slots, the damaged one included (X-1 item 6, pinned). The order is the test's, not the scheduler's (the repair of the review's F-R2 at `157c9cab`, where alpha could end first and beta be cancelled before its policy, and the test passed with no refusal): alpha's worker answers only once beta is blocked in its policy (`FAfterBetaBlocks`; it waits holding one of the three agent slots the width gives, and beta holds another). The test asserts that beta entered its policy, that the policy handed back the checkout damaged with its registration not whole (`FBetaRead`), and that beta's pipeline never asked for its review inputs (`FInputsOf`): it ended at its refused read. A refused completion is read by nothing after the halt, by design, so the refusal is observed where it is made.

## `fn a_kept_slots_read_waits_on_a_contended_registry_through_the_coordinators_hooks() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-CONC-3 (under U): closure 2's `kept_instances` read meets a foreign registration whose `gitdir` cannot be read (the third coordinator access after the retained fold); the coordinator serves the other pipeline during the wait, no wait sleeps on its thread, and the retry then refuses for the pruned slot.

## `struct FEvents {`

Event hooks injecting one append error at the first `Event.Append` `Synced` point after the refusing pipeline is blocked, and releasing it: its completion is then received after the fold is poisoned.

## `struct FAppendFailing {`

`FHooks` with `FEvents` for the event hooks.

## `fn a_refusal_arriving_after_an_append_error_is_discarded_on_the_poisoned_fold() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12 — T-CONC-4: the append error is the command's error; beta's refusal is discarded on the poisoned fold (counted, named by no warning); the next resume follows the surviving prefix, and P-1's guard keeps beta's slot, whose `index` is gone, until the operator removes it; then the run ends.

## `fn f_remove_tree_by_hand(root: &std::path::Path) {`

The operator removing a kept directory.

## `fn f_alpha_after_beta_blocks(blocked: FSignal, beta: FPolicy) -> FPolicy {`

The first task's policy waits until beta is blocked, so that an append follows; beta's act as given.

## `fn a_verification_refused_after_its_passes_were_charged_records_them_under_o14b() {`

The external-prune follow-up (F) — `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its design's §12.6 — T-O14B-F, which replaces T-O14A-F now that O14(b) is on the base: refused at placement 2 at pass 1 (unavailable, and passing), after both passes were charged, at width 1 and at width 3. Before the command ends the log holds one `merge_verification_charged`, for the refused sequence, with both passes at their cost, and no terminal for it; the live spend and the replay agree, the record carrying the two passes. The next resume appends that sequence's `merge_verification_interrupted` and no second record, and its replay counts both passes once. Width 1 records in `TopologyRun::integrate`, width 3 in `Coordinator::integrate` or `finish`, the points `design/26` names. Mutation M-C1e (the review's check before `account.charge`) leaves pass 1 uncharged and unrecorded, so the record holds one pass and this test is red.

## `struct FCostedReviewStub {`

Both verification passes cost 0.25; the target one is `FReviewStub`'s.

## `fn f_drive_costed(`

T-O14B-F's run: two verification reviewers. At width 3 the stub reviews every verification of the first command; at width 1 that command ends at gamma's pick, as `f_drive_subject`'s does, and the resumed process steps the run with the stub until the refusal.

## `fn f_promoting_left(tag: &str, keep_the_store: bool) -> Wide {`

The crash prefix the fresh regular lens's P1 at `f1842dcd` names (T-PROM-*). One task, at width 1, in a durable run, whose candidates ref's creation fails before it is made, so the command ends with `candidate_prepared` durable and `task_candidate_created` not: a `Promoting` generation whose populated task worktree, intent and pin stand. With `keep_the_store`, a registration the engine did not make (`f_keep_the_store`) keeps the store when the task's entry is pruned.

## `fn f_pruned_promoting_checkout(wide: &Wide) -> PathBuf {`

The promoting task's checkout, given a file its candidate does not pin, and its whole registration entry pruned by a prune no engine process started (`f_prune_own_entry`). With the store's last entry pruned, Git removes the store too.

## `fn f_resume_fresh(`

A fresh-process resume of the left run, as `try_resume_over` makes it: what step (f) finished, or the resume's refusal, and the durable events either way. It also returns the `WideEnv`, which owns the fixture and so the scratch tree the checkout is in. The resumed run is dropped, as the resumed process would end; the env is not. Every caller holds it through its last filesystem and event assertion and drops it after them, so the guard reclaims the tree only once those assertions are done, or as a failing one unwinds. An earlier form dropped the env here, so the tree was reclaimed before the callers' comparison, which then failed listing the checkout (`NotFound`).

## `fn f_promotion_kept(`

P-6′ at recovery's completion of a promotion: the resume refuses with the kept-slot refusal naming the completion of the promotion and the kept checkout, before `task_candidate_created` and before `run_resumed`, and the checkout and every byte it holds are kept. A resume that finished the promotion over the pruned populated slot is the failure this states. Its callers hold the fixture's owner until it returns, so the comparison reads the live checkout; T-PROM-6 shows that the comparison fails once a kept file is gone.

## `fn a_resume_keeps_a_promoting_generations_pruned_populated_slot_with_the_store_gone() {`

T-PROM-1, the witness: the fresh regular lens's P1 sequence, the `Promoting` crash prefix, the populated slot's entry pruned and the store gone with it. At `f1842dcd` recovery's step (f) finishes the promotion and closure 3's arm removes the checkout with what it holds; repaired, `candidate::recovery_for` keeps it and the resume refuses.

## `fn a_resume_keeps_a_promoting_generations_pruned_populated_slot_while_the_store_stays() {`

T-PROM-2, the positive preservation case with the store kept: the same prefix, the entry pruned while another registration keeps the store. At `f1842dcd` the forced removal takes the checkout as well; repaired, it is kept and the resume refuses, naming it.

## `fn the_kept_slot_comparison_fails_once_a_kept_file_is_removed_by_hand() {`

T-PROM-6, the liveness control of T-PROM-1's and T-PROM-2's comparison: the same prefix as T-PROM-1 and the same resume, which refuses; then one file the slot kept, asserted present, is removed by hand through the fixture's `remove_file` while the fixture lives (a direct `std::fs::remove_file` is a denied method in `clippy.toml`). `f_promotion_kept` over that refusal must then fail at its comparison and not at an earlier check, so the comparison reads the live checkout and is not vacuous. At `f1842dcd` the resume finishes the promotion, and this control fails at its first step, the refusal it expects.

## `fn a_resume_finishes_the_promotion_once_the_operator_has_removed_its_kept_slot() {`

T-PROM-3, the release control (G-2): the operator has taken what the slot held and removed the directory, so the check finds nothing held and the resume finishes the promotion (`task_candidate_created`, then `run_resumed`). The same before and after the repair. Its assertions run while the fixture lives. The last, that nothing is left at the slot, shows that the operator's removal stands and nothing recreated the slot; it is not evidence that the product removed anything.

## `fn a_live_promotion_still_removes_its_pruned_populated_slot_with_the_store_gone() {`

T-PROM-4, the live-cleanup control: the entry is pruned, the store with it, at the candidates ref's creation of a live promotion; the promotion is completed live and its reclaim removes the checkout with what it holds (Choice A's deletion outside recovery), and the run completes. The same before and after the repair: the live promotion does not read `candidate::recovery_for`.

## `fn the_run_end_closure_still_removes_a_promoting_generations_pruned_populated_slot() {`

T-PROM-5, the run-end control: `promoting_completed_by_the_closure_at_run_end` with alpha's entry pruned and a file in its checkout after the decline is ingested. The halting closure completes the promotion through `run::complete_promotions`, which reads `candidate::owed_for`, and removes the checkout with what it holds (P-6′'s run-end clause). The same before and after the repair; it would turn red if run-end closure read `candidate::recovery_for`.

## `fn a_clean_run_resumed_under_a_linked_registry_is_refused_until_the_link_is_gone() {`

R-C-REGISTRY part 1 on the resume path, and B-1 and B-2 for it: red at the
bound master, where the resume goes ahead, and green here. A schema-4 run is
created clean, its `worktrees` registry is then made a link into another
directory (a junction on Windows), and the next coordinator, which inherits no
`GIT_COMMON_DIR`, resumes it through `try_resume_over`, whose first call is
`WorkspaceManager::derive`. The resume refuses, naming the entry and its
target; its log is byte for byte as it was, the container trace it was handed
records no inspection or probe, and no slot, intent or registration is made in
the execution root or in the linked directory. Once the link is removed the same
environment resumes and the run completes.

The test asserts first that this process inherits no `GIT_COMMON_DIR`: with
one inherited the layout is part 2's, accepted by name, and the refusal would
rightly not fire.

## `fn a_relative_object_directory_refuses_derive_with_the_resumes_arguments() {`

R-C-RELSTORE at the `derive` boundary, with the arguments a resume gives it: red at the bound
master, where `derive` does not refuse, and green here. **It is not a full resume witness.** A
schema-4 run is created clean; an isolated child, which inherits `GIT_OBJECT_DIRECTORY=../objects`,
calls `WorkspaceManager::derive` with the base, private root, run id and next incarnation that
`try_resume_over` passes as its first call, and is refused; the run's log is unchanged and no slot
or intent is made. This process, whose environment carries no such value, then resumes the run,
and that clean resume completes.

**The full-resume gap.** No witness drives `try_resume_over` itself under an inherited relative
name. This process's own environment is never set, so the condition can live only in a child; and
in a child the scaffold's environment cannot be rebuilt over the run without its fixture deriving a
manager on adoption, which the same condition refuses first. What covers the gap is the census: the
resume's first call is `derive`, `derive` is the manager's only constructor, and RELSTORE runs in
it before the manager's first Git child. The in-process
`a_clean_run_resumed_under_a_linked_registry_is_refused_until_the_link_is_gone` exercises
`try_resume_over`'s own refusal, for R-C-REGISTRY.
