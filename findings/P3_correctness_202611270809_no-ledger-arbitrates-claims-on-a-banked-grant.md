---
id: PR441-GRANT-LEDGER-WITHDRAWN
severity: P3
disposition: deferred
category: correctness
pr: 441
reviewed_sha: c1e0f3ae9d04de53d0d0ab5c84d8595d4d2b28ce
location: src/capacity_planner.rs:1467
provenance: introduced_by_feature
first_bad:
guard: the handler stage that first redeems a grant
---

# No ledger arbitrates claims on a banked grant

Filed deferred under ruling r84, which withdrew #441's simulated ledger, `src/capacity_planner.rs:1243-2031` at the reviewed commit, together with every test, fixture and helper that needed it. The only part of the planner left is the offline forecast, `plan`, and nothing calls it. The sequence below is reasoned from the code that remains, not run.

## Failure sequence

    the handler stage that first redeems a grant takes a `Redeem` from `plan` for an account
    -> a second run on that account gets the same `Redeem`, or claims while the first claim's reply is unknown
    -> nothing records the first claim, settles it from the provider's reply and the next inventory, or refuses the second
    -> nothing holds the capacity the redemption restores until balances and timestamps are read again, as design §13 requires
    -> one reset is claimed twice, or restored capacity is allocated before the re-read

The head redeems nothing, so the gap fails nowhere today. Nothing calls `plan`, and no stage redeems a grant or allocates restored capacity. The sequence starts at the first stage that does.

## What is established

- `plan` is pure and keeps no state between calls, so the same `Input` always gives the same plan (`docs/internals/capacity_planner.md`, *Module*). Within one call, at most one of an account's grants is `Redeem`. Every other grant that would have been `Redeem`, `Defer` or `LetExpire(NoUnlock)` becomes `Unknown(AwaitsReRead)`, while `LetExpire(NotRedeemable)` and every other `Unknown` stand (*Tie behavior*). Nothing carries that across two calls or two runs.
- Design §13 requires actual balances and timestamps to be read again after an authorized redemption, before restored capacity is allocated. Nothing at this head allocates capacity, so nothing enforces that.
- The withdrawn ledger did both of these in memory, and two review rounds found the thirty-nine defects listed below. It registered claims on a grant, settled them from replies and inventories, issued permits against window balances, and marked a restored window to be read again. Round one's repairs introduced seven of round two's rows (`fix_regression`).

## The withdrawn rows

Every row is withdrawn together with the code it was filed against. Round one's repairs went with that code, and so did every regression test from either round.

### Round one: fifteen rows, #441's ledger at `bad48716`

Locations are at `bad48716`.

- `PR441-LEDGER-PENDING-EXHAUSTION` (P2, `src/capacity_planner.rs:1674`). A pending claim suppresses fresh evidence that capacity decreased.
  A Fable claim marks a session with 80% remaining for re-read -> a newer observation reports 100% used before the reply -> the ledger discards it as delayed -> another run receives a permit drawing 10% from the exhausted session. Witness: `pending_claim_must_honor_observed_exhaustion`.
- `PR441-LEDGER-RESTART-WITHOUT-REFRESH` (P2, `src/capacity_planner.rs:1701`). A restarted Codex window becomes fully spendable without its next refresh.
  Claim an exhausted covered Codex session -> receive Accepted -> observe 0% used and a reduced grant count, with the refresh timestamp absent -> the ledger clears the re-read requirement and permits a 100% draw. Witness: `restarted_codex_window_needs_its_refresh_before_spending`.
- `PR441-LEDGER-RETAINED-CLOCK-ERASED` (P2, `src/capacity_planner.rs:1670`). A partial snapshot erases the refill boundary of a window that kept its clock.
  A Claude weekly window has a fixed refill, or an already-full Codex window keeps its refill across redemption -> a newer balance omits the timestamp -> Unreported replaces the known boundary -> after that boundary passes, the old balance still funds a permit. Witnesses: `missing_timestamp_cannot_erase_claudes_fixed_weekly_boundary`, `full_codex_allowance_keeps_its_clock_on_partial_refresh`.
- `PR441-LEDGER-OLD-PLAN-PERMIT` (P2, `src/capacity_planner.rs:1692`). A plan change leaves the previous plan's window balances allocatable.
  The old plan reports 100% weekly capacity remaining -> a newer session reading identifies a new plan -> the new plan has supplied no weekly reading -> a permit draws the entire weekly allowance from the old plan. Witness: `changed_plan_cannot_allocate_old_plans_weekly_allowance`.
- `PR441-LEDGER-RELEASE-REISSUES-CONSUMED-QUOTA` (P2, `src/capacity_planner.rs:1409`). Releasing a completed permit makes its consumed quota available again.
  Observe a restored balance of 100% -> issue it to A -> A uses the allowance and releases its permit -> before another provider reading, B receives the same 100%. Witness: `returning_a_used_permit_cannot_reallocate_the_same_reading`.
- `PR441-LEDGER-ZERO-BALANCE-PERMIT` (P3, `src/capacity_planner.rs:1791`). An exhausted window can issue repeated positive permits.
  Observe 100% used -> request a positive draw of 0.000000001% twice -> adding the tolerance to zero admits both draws, while usable continues to clamp the balance to zero. Witness: `an_exhausted_window_cannot_issue_positive_dust_permits`.
- `PR441-LEDGER-FOREIGN-RECOMMENDATION` (P2, `src/capacity_planner.rs:1532`). One account's recommendation can authorize another account's empty grant.
  A has one reset and B has zero at the same generation and second -> obtain Redeem from A -> submit that basis with B's correct grant reference -> B accepts because the basis carries no account or grant identity. Witness: `recommendation_is_bound_to_the_account_and_grant`.
- `PR441-LEDGER-BASIS-OUTLIVES-REFILL` (P2, `src/capacity_planner.rs:1533`). A cached redemption recommendation survives the refill that justified it.
  Read an exhausted weekly window that refills in 120 seconds -> obtain Redeem -> advance 121 seconds within the one-hour freshness limit -> the old recommendation still authorizes a claim. Witness: `recommendation_cannot_survive_a_natural_refill`.
- `PR441-LEDGER-LATE-ACCEPTANCE` (P2, `src/capacity_planner.rs:1625`). A late acceptance leaves the same grant available to a second run.
  A claims the only grant -> an Unknown reply and an unchanged count release it -> Accepted arrives for A before B claims -> the ledger ignores that acknowledgment and authorizes B without a newer inventory read. Witness: `late_acceptance_cannot_leave_grant_available_to_next_run`.
- `PR441-LEDGER-SAME-SECOND-SETTLEMENT` (P2, `src/capacity_planner.rs:1754`). A snapshot captured before a claim can pass the post-reply check.
  Capture count 1 -> claim and receive Unknown in that same Unix second -> deliver the earlier snapshot one second later -> timestamp equality releases the reservation and a second run claims it. Witness: `snapshot_captured_before_reply_in_same_second_cannot_release`.
- `PR441-LEDGER-CONFLICT-ORDER` (P2, `src/capacity_planner.rs:1722`). Conflicting inventories settle the grant according to arrival order.
  After a refused claim, deliver count 0 and count 1 for the same second -> deliver a later unambiguous count 1 -> the final state is Spent if 0 arrived first and Banked if 1 arrived first. Witness: `conflicting_inventory_cannot_irreversibly_settle_by_arrival_order`.
- `PR441-LEDGER-GRANT-ROW-IDENTITY` (P2, `src/capacity_planner.rs:1741`). Reordering inventory rows can fabricate a grant spend.
  Two weekly rows share a use-by time and have counts 2 and 1 -> the claim is refused -> the next inventory lists the identical rows in reverse order -> matching the first kind-and-expiry pair compares 2 with 1 and records Spent. Witness: `reordering_indistinguishable_inventory_entries_is_not_a_spend`.
- `PR441-LEDGER-INCOMPLETE-INVENTORY-ROW` (P2, `src/capacity_planner.rs:1738`). A row missing its expiry is treated as proof the grant was spent.
  After Refused, report the same weekly grant with count 1 and its expiry omitted -> the reader preserves the unknown expiry -> the ledger reads the failed identity match as disappearance and records Spent -> a later complete row produces Redeem, but the claim still returns Refused(Spent). From the regression review, which named no witness.
- `PR441-LEDGER-EXPIRY-AS-SPEND` (P3, `src/capacity_planner.rs:1755`). A grant disappearing at expiry is attributed to a refused redemption.
  Claim shortly before expiry -> receive Refused -> take the next inventory after expiry, when the grant is absent -> the ledger records that the requester spent it. Witness: `expiry_after_refusal_is_not_evidence_of_a_spend`.
- `PR441-LEDGER-REMAINING-RESETS` (P2, `src/capacity_planner.rs:1761`). Spending one reset permanently disables the remainder of its inventory row.
  Claim from a row with count 2 -> observe a successful restoration and count 1 -> fresh readings later show exhaustion and confirm the remaining reset -> recommend returns Redeem, but claim always refuses Spent. Witness: `a_second_observed_reset_in_the_same_inventory_row_can_be_claimed`.

### Round two: twenty-four rows, at the reviewed commit

Locations are at `c1e0f3ae`. The last three rows come from the regression review.

- `PR441-LEDGER-SECOND-RESET-BEFORE-WINDOW-READ` (P2, `src/capacity_planner.rs:1968`; `fix_regression`, `325afb50`). The remaining reset can be claimed using exhaustion observed before the first reset.
  A claims from a count of two and receives Accepted -> inventory falls to one with no weekly reading -> the weekly window still awaits a re-read, but recommend uses its old exhaustion -> B immediately claims the second reset. Reproduction: `extra_next_reset_needs_its_own_window_evidence` returned Redeem and Claimed(2) with the weekly re-read flag still set.
- `PR441-LEDGER-LATE-ACCEPTANCE-WHILE-HELD` (P2, `src/capacity_planner.rs:1772`; `introduced_by_feature`, `2669c177`, retained after `d935b3e3`). A late acceptance is lost while a second claimant holds the grant.
  A receives Unknown and an unchanged count releases it -> B claims -> Accepted arrives for A while B holds the grant -> B receives Refused and the count remains one -> C claims despite A's unresolved acceptance. Reproduction: `extra_late_acceptance_during_next_claim` returned Ignored(Stale) for A's acceptance and registered all three claimants against one reset.
- `PR441-LEDGER-DECREMENT-ATTRIBUTION` (P2, `src/capacity_planner.rs:1960`; `fix_regression`, `325afb50`). An account-wide decrement is attributed to a specific claimant without evidence.
  A claims from two resets and receives Unknown -> a different client consumes one, so the next inventory reports one -> the ledger records A in spends -> A's later Accepted is discarded as already accounted for -> B claims without an inventory reading after that acceptance. Reproduction: `extra_independent_decrement_does_not_identify_claimant` supplied the unattributed count decrease and a fresh exhausted-window reading; the later acknowledgment returned Ignored(Stale) and B received Claimed(2).
- `PR441-LEDGER-UPGRADED-ACK-TIME` (P2, `src/capacity_planner.rs:1753`; `introduced_by_feature`, `2669c177`). Upgrading an outstanding reply to Accepted keeps the earlier reply's timestamp.
  A receives Unknown at second 2 -> a count of one is captured at 3 -> A receives Accepted at 4 -> the earlier snapshot arrives at 5 and settles the acceptance because the stored reply time is still 2. Reproduction: `extra_accepted_upgrade_starts_a_new_inventory_barrier` returned Ignored(Duplicate), stored Accepted at second 2 and banked the remaining reset from the second-3 snapshot.
- `PR441-LEDGER-CONFLICT-AFTER-STATE-CHANGE` (P2, `src/capacity_planner.rs:1910`; `introduced_by_feature`, `2669c177`, retained after `d935b3e3`). An intervening claim or expiry defeats rollback of conflicting inventory.
  A receives Refused -> counts one and zero have the same observation second -> B attempts to claim between their deliveries -> rollback requires the state produced by the first count still to be current -> reversing delivery order changes the recorded spender. Automatic expiry can break the same check without a second claimant. Reproduction: `extra_claim_between_conflicting_inventory_reads` recorded B versus A as spender; `extra_conflicting_snapshot_delivered_after_expiry` produced Expired versus Spent from the same readings and the same final unambiguous count.
- `PR441-LEDGER-WINDOW-READ-IN-REPLY-SECOND` (P2, `src/capacity_planner.rs:1844`; `introduced_by_feature`, `2669c177`, retained after `aaa2cb4e`). A window snapshot captured before the reply can authorize restored capacity.
  Claim an exhausted weekly window -> capture a 0%-used snapshot before the reply in the same Unix second -> receive Unknown -> deliver that snapshot one second later -> timestamp equality clears the re-read requirement and funds a 100% permit. Reproduction: `extra_window_snapshot_from_reply_second` received Permitted(2). The inventory settlement check already required a strictly later observation second.
- `PR441-LEDGER-PARTIAL-READ-WATERMARK` (P2, `src/capacity_planner.rs:1857`; `fix_regression`, `aaa2cb4e`). A delayed complete refresh overwrites evidence of more recent usage.
  After Accepted on an exhausted Codex session, receive 80% used at second 4 without a refresh time -> the ledger drops the observation and its timestamp -> deliver 0% used with a refresh captured at second 3 -> the ledger issues 100% although the newer reading leaves at most 20%. Reproduction: `extra_partial_refresh_supersedes_older_full_refresh` received Permitted(2) for the full 100% draw.
- `PR441-LEDGER-LATE-ACK-WINDOW-CLOCK` (P2, `src/capacity_planner.rs:1762`; `fix_regression`, `d935b3e3`). Reopening a released claim on late acceptance leaves its windows cleared for spending.
  Unknown plus an unchanged count releases a Codex claim and an exhausted session reading supplies its old clock -> late Accepted reopens the claim without marking the window for another read -> a 0%-used reading omits the new refresh -> the old clock funds a 100% permit. Reproduction: `extra_late_acceptance_requires_new_window_clock` received Permitted(2) with the pre-acceptance refresh and awaiting_re_read false.
- `PR441-LEDGER-FULL-WINDOW-USED-BEFORE-REPLY` (P2, `src/capacity_planner.rs:1704`; `introduced_by_feature`, `2669c177`). Codex coverage is classified using fullness at claim time even when use changes before redemption.
  The covered weekly allowance is full when a full reset is claimed for the session -> a pre-reply reading reports weekly 60% used -> Accepted arrives -> weekly reports 0% used without its new refresh -> the ledger treats weekly as an allowance that stayed full and issues 100% against its old clock. Reproduction: `extra_below_full_at_redemption` observed 40% usable before acceptance and then received Permitted(2) for 100% with the old weekly refresh.
- `PR441-LEDGER-CLAIM-ERASES-LIVE-BOUNDARY` (P2, `src/capacity_planner.rs:1693`; `introduced_by_feature`, `2669c177`). A pending claim extends the lifetime of an observed balance.
  A session has 80% remaining and a reported refill at second 120 -> a Fable claim replaces that clock with AwaitingReRead -> no reply or reading arrives -> at second 121 the pre-claim balance still funds an 80% draw. Reproduction: `extra_pending_preclaim_clock_passes` returned Blocked(Refilled) on the control ledger and Permitted(2) on the otherwise identical ledger with a pending claim.
- `PR441-LEDGER-PENDING-WINDOW-CONFLICT` (P2, `src/capacity_planner.rs:1853`; `introduced_by_feature`, `2669c177`, retained after `aaa2cb4e`). A pending claim suppresses same-second evidence of exhaustion.
  A session reports 20% used -> a claim marks the session -> another reading from that same observation second reports 100% used -> the marked-window path discards the contradiction as delayed -> another request draws 80%. Reproduction: `extra_pending_same_second_exhaustion` counted one delayed reading, zero conflicts and issued Permitted(2).
- `PR441-LEDGER-ACTIVE-SESSION-WITHOUT-CLOCK` (P2, `src/capacity_planner.rs:1848`; `introduced_by_feature`, `2669c177`, retained by the rolling-clock exception in `aaa2cb4e`). A restarted Claude session with positive usage can fund permits without a refresh time.
  An exhausted session receives Accepted -> the next reading reports 25% used and no refresh -> the rolling-window exception clears the mark and authorizes 75%. Positive usage rules out the fixture note's explanation that the session is waiting for its first use. Reproduction: `extra_active_claude_session_needs_clock` returned Permitted(2) with refill Unreported and awaiting_re_read false.
- `PR441-LEDGER-MISSED-FIXED-WEEK` (P2, `src/capacity_planner.rs:1835`; `introduced_by_feature`, `2669c177`, retained after `aaa2cb4e`). A gap in readings erases Claude's recurring weekly boundary.
  Record the fixed weekly boundary -> miss one refill -> one minute before the following weekly boundary receive 10% used without a timestamp -> the ledger changes the clock to Unreported -> a permit spends 90% from the previous week one second after the boundary. Reproduction: `extra_fixed_weekly_boundary_after_a_skipped_week` received Permitted(1) from the preceding week's reading.
- `PR441-LEDGER-BASIS-ALIASES-SNAPSHOT` (P2, `src/capacity_planner.rs:1665`; `introduced_by_feature`, `2669c177`, retained after `e5f3da05`). Two ledger snapshots for the same grant can exchange incompatible recommendations.
  Initialize two ledgers for the identical account, kind, expiry, generation and second, one with count one and the other with count zero -> pass the first ledger's Redeem to the second -> the second reserves a grant it knows has zero resets. Reproduction: `extra_same_grant_different_snapshot` produced Reserved with resets_left Some(0); the receiving ledger's own decision was LetExpire(NotRedeemable).
- `PR441-LEDGER-BASIS-OUTLIVES-EVIDENCE` (P2, `src/capacity_planner.rs:1666`; `introduced_by_feature`, `2669c177`, retained after `e5f3da05`). Creating a recommendation restarts the freshness period for its underlying evidence.
  Read windows and inventory at second zero with a one-hour freshness limit -> obtain Redeem at second 3599 -> claim at 3601 before any refill -> the two-second-old recommendation passes although its inputs have expired. Reproduction: `extra_stale_underlying_evidence` obtained no decision from a fresh recommendation at 3601, then received Claimed(1) with the cached recommendation.
- `PR441-LEDGER-PARTIAL-PLAN-CHANGE-BASIS` (P2, `src/capacity_planner.rs:1817`; `introduced_by_feature`, `2669c177`, partially addressed by `aaa2cb4e`). A plan change reported without a usage percentage leaves old claim authority intact.
  Cache Redeem on plan A -> receive a newer reading naming plan B with no percentage -> the ledger records the plan for permit checks but returns Missing before recording it for the forecast or advancing the generation -> the old basis still claims. Reproduction: `extra_missing_balance_changes_plan` saw PlanChanged for the weekly permit balance, a fresh Redeem, and Claimed(1) with the pre-change basis.
- `PR441-LEDGER-UNKNOWN-KIND-AS-SPEND` (P2, `src/capacity_planner.rs:1951`; `introduced_by_feature`, `2669c177`, retained after `325afb50`). A grant row with an unknown kind is mistaken for disappearance.
  A weekly claim is refused -> the next inventory reports one reset with the same expiry and an unknown kind -> the partial-row guard checks only for a missing expiry on a known kind -> the ledger records Spent and refuses a later complete positive row. Reproduction: `extra_ambiguous_kind_does_not_prove_spend` recorded Spent by A and returned Refused(Spent) after weekly count one was confirmed.
- `PR441-LEDGER-FIRST-MATCH-SHADOWS-RESET` (P2, `src/capacity_planner.rs:1556`; `introduced_by_feature`, `2669c177`, retained after `325afb50`). A zero-count row can hide a usable reset with the same identity.
  Supply two weekly rows with the same expiry and counts zero and one -> settlement correctly totals one -> recommend takes only the first matching row's decision -> putting zero first makes the usable reset unclaimable. Reproduction: `extra_first_matching_row_hides_usable_reset` got Redeem with the positive row first and Refused(NotRecommended(LetExpire(NotRedeemable))) with the same rows reversed.
- `PR441-LEDGER-SPENT-IGNORES-REPLENISHMENT` (P2, `src/capacity_planner.rs:1980`; `introduced_by_feature`, `2669c177`, retained after `325afb50`). A confirmed positive count cannot reopen a previously exhausted inventory row.
  Settle the last reset at count zero -> a newer pre-expiry inventory positively reports one reset under the same account, kind and expiry -> the planner recommends redemption but Spent absorbs the update and permanently refuses the claim. Reproduction: `extra_later_replenishment_can_be_claimed` obtained Redeem from the fresh positive inventory and Refused(Spent) from claim.
- `PR441-LEDGER-EXPIRED-ZERO-AS-SPEND` (P3, `src/capacity_planner.rs:1959`; `introduced_by_feature`, `2669c177`, retained after `325afb50`). An expired row retained at count zero is attributed to the refused claimant.
  A claims before expiry and receives Refused -> at the use-by second the provider retains the expired row with zero resets -> the expiry exception handles only an absent row -> the ledger records A as having spent it. Reproduction: `extra_expired_row_zero_after_refusal` returned Spent by A at the expiry second. Round one's fixture covered an empty inventory at expiry.
- `PR441-LEDGER-SAME-SECOND-ROW-ORDER` (P3, `src/capacity_planner.rs:1903`; `introduced_by_feature`, `2669c177`, retained after `325afb50`). Reordering identical inventory contents creates a false same-second conflict.
  Read two same-key rows with counts one and two -> receive the same rows in reverse order for the same observation second -> vector equality marks the inventory conflicting -> the recommendation disappears until another reading. Reproduction: `extra_inventory_order_is_not_a_conflict` counted one conflict and returned no decision for the reordered inventory.
- `PR441-LEDGER-COMPATIBLE-READING-ORDER` (P2, `src/capacity_planner.rs:1834`; `fix_regression`, `aaa2cb4e`). The order of two compatible readings changes available capacity.
  Supply two weekly readings for the same second, both 40% used, only one with the refresh -> delivering the complete reading first leaves 60% usable -> delivering it second yields `Blocked::Conflicting`.
- `PR441-LEDGER-NEW-PLAN-INHERITS-CLOCK` (P2, `src/capacity_planner.rs:1834`; `fix_regression`, `aaa2cb4e`). A new plan inherits the old plan's clock.
  Plan A reports a weekly refill at second 120 -> at second 60, plan B reports 0% used and omits its refill -> the ledger assigns A's timestamp to B -> B is blocked at second 121 as `Refilled`. The immediate parent kept B's clock `Unreported`.
- `PR441-LEDGER-ADVICE-IGNORES-COMPLETED-DEBIT` (P2, `src/capacity_planner.rs:1538`; `fix_regression`, `4e533a0a`). Grant advice ignores the debit for completed work.
  Start at 80% weekly usage -> A consumes and returns a permit for the remaining 20%, and `usable(Weekly)` correctly returns zero -> with a one-hour job needing 10%, a reset expiring in ten minutes and the weekly refill 72 hours away, `recommend` returns `LetExpire(NoUnlock)` -> its input still contains the pre-permit balance.

## What the change that takes this up should do

- **Design in §13 before code.** Write the ledger's rules into design §13, and get them agreed, before writing any of its code. The rules cover what a claim is and who may hold one, what a provider's reply and a later inventory each establish about it, when restored capacity may be allocated, and what a permit debits and until when.
- **Order evidence by a caller-supplied sequence, never by whole seconds.** Every reading, inventory, reply and claim carries a sequence the caller assigns. "Before", "after" and "the same" are decided by that sequence, never by comparing or equating `observed_at_unix`. `PR441-LEDGER-SAME-SECOND-SETTLEMENT`, `PR441-LEDGER-WINDOW-READ-IN-REPLY-SECOND`, `PR441-LEDGER-UPGRADED-ACK-TIME` and `PR441-LEDGER-PENDING-WINDOW-CONFLICT` each depend on two pieces of evidence sharing a Unix second, or on a later second being taken as later evidence.
- **One rule for a partial reading.** A reading or inventory row that omits a field (a percentage, a refresh time, a kind or an expiry) is read by one stated rule. That rule applies wherever the evidence is used: the forecast, permit balances and settlement alike. In `PR441-LEDGER-PARTIAL-PLAN-CHANGE-BASIS`, for example, a plan named without a percentage was recorded for permits but not for the forecast.
- **Settlement as a function of the evidence set, whatever its arrival order.** A grant's settled state, and the claimant it is attributed to, are computed from the set of evidence received. Any arrival order of the same evidence, and any order of rows within one inventory, gives the same answer. `PR441-LEDGER-CONFLICT-ORDER`, `PR441-LEDGER-CONFLICT-AFTER-STATE-CHANGE`, `PR441-LEDGER-COMPATIBLE-READING-ORDER`, `PR441-LEDGER-SAME-SECOND-ROW-ORDER`, `PR441-LEDGER-GRANT-ROW-IDENTITY` and `PR441-LEDGER-FIRST-MATCH-SHADOWS-RESET` each changed a result by reordering the same evidence.
- **Every sequence of both rounds as a fixture.** Each of the thirty-nine failure sequences above becomes a fixture of that change. Where a row has a witness, the fixture takes its name without the `extra_` prefix.
