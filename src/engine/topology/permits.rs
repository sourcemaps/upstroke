//! Extended notes: `docs/internals/engine/topology/permits.md`

use crate::error::UpstrokeError;
use crate::runner::InvocationId;
use crate::topology::registry::TaskKey;

use super::identity::{
    Admission, InvocationEnd, InvocationLedger, ReservationKind, Reservations, SlotLimits, SlotPair,
};
use super::select::{Entitlements, Standing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutDown {
    pub reservations: usize,
    pub pending: usize,
}

#[derive(Debug, Default)]
pub struct PermitBroker {
    reservations: Reservations,
    invocations: InvocationLedger,
}

impl PermitBroker {
    #[must_use]
    pub fn new(slots: SlotLimits) -> Self {
        Self {
            reservations: Reservations::new(),
            invocations: InvocationLedger::with_limits(slots),
        }
    }

    #[must_use]
    pub fn for_run(derived: &Entitlements) -> Self {
        Self::new(SlotLimits::defaulted(derived.max_parallel()))
    }

    #[must_use]
    pub fn for_pipelines(slots: SlotLimits) -> Self {
        Self {
            reservations: Reservations::new(),
            invocations: InvocationLedger::for_pipelines(slots),
        }
    }

    pub fn reserve(
        &mut self,
        derived: &Entitlements,
        key: TaskKey,
        kind: ReservationKind,
    ) -> Result<(), UpstrokeError> {
        self.reservations.reserve(key, kind, derived)
    }

    pub fn convert(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {
        self.reservations.convert(key, kind)
    }

    pub fn cancel_reservation(
        &mut self,
        key: TaskKey,
        kind: ReservationKind,
    ) -> Result<(), UpstrokeError> {
        self.reservations.cancel(key, kind)
    }

    pub fn register(
        &mut self,
        standing: &Standing,
        invocation: &InvocationId,
        slots: Option<SlotPair>,
    ) -> Result<Admission, UpstrokeError> {
        match slots {
            None => {
                self.invocations.register(invocation)?;
                Ok(Admission::Runnable)
            }
            Some(pair) => self
                .invocations
                .register_slotted(invocation, pair, standing),
        }
    }

    pub fn complete(
        &mut self,
        invocation: &InvocationId,
    ) -> Result<Vec<InvocationId>, UpstrokeError> {
        self.invocations.complete(invocation)
    }

    pub fn cancel(
        &mut self,
        invocation: &InvocationId,
    ) -> Result<Vec<InvocationId>, UpstrokeError> {
        self.invocations.cancel(invocation)
    }

    pub fn end(
        &mut self,
        invocation: &InvocationId,
        end: &InvocationEnd,
    ) -> Result<Vec<InvocationId>, UpstrokeError> {
        self.invocations.end(invocation, end)
    }

    pub fn shut_down(&mut self) -> ShutDown {
        ShutDown {
            reservations: self.reservations.cancel_all(),
            pending: self.invocations.withdraw_pending(),
        }
    }

    #[must_use]
    pub const fn reservations(&self) -> &Reservations {
        &self.reservations
    }

    #[must_use]
    pub const fn invocations(&self) -> &InvocationLedger {
        &self.invocations
    }

    pub fn halves(&mut self) -> (&mut Reservations, &mut InvocationLedger) {
        (&mut self.reservations, &mut self.invocations)
    }

    #[must_use]
    pub fn duplicates(&self) -> u32 {
        self.reservations
            .duplicates()
            .saturating_add(self.invocations.duplicates())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reservations.is_empty()
            && self.invocations.registered() == 0
            && self.invocations.slots().is_empty()
    }

    #[must_use]
    pub fn balances(&self) -> bool {
        self.reservations.balances() && self.invocations.balances()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::engine::topology::identity::{
        AttemptIdentities, PreflightIdentities, SequenceIdentities,
    };
    use crate::engine::topology::select::Holds;
    use crate::engine::topology::settle::tests::{
        ALEPH, BET, GIMEL, apply, dispatch, ev, in_flight, inputs, retained_generation, run_started,
    };
    use crate::topology::events::{
        AttemptNumber, GenerationId, RunStarted4, SequenceId, TopologyEventBody, TopologyLimits,
    };
    use crate::topology::fold::TopologyFold;

    const AGENT: &str = "claude";
    const OTHER_AGENT: &str = "codex";
    const POOL: &str = "anthropic";
    const OTHER_POOL: &str = "openai";

    fn started_at_width(max_parallel: u32) -> TopologyFold {
        let base = run_started();
        let limits = TopologyLimits {
            max_parallel,
            ..base.limits
        };
        let mut fold = TopologyFold::new(inputs());
        apply(
            &mut fold,
            &ev(TopologyEventBody::RunStarted {
                data: Box::new(RunStarted4 { limits, ..base }),
            }),
        );
        fold
    }

    fn three_in_flight() -> TopologyFold {
        let mut fold = started_at_width(3);
        for key in [ALEPH, BET, GIMEL] {
            in_flight(&mut fold, key, 0);
        }
        fold
    }

    fn of(key: TaskKey) -> AttemptIdentities {
        AttemptIdentities::new(key, GenerationId(0), AttemptNumber(1))
    }

    fn pair(agent: &str, pool: Option<&str>) -> SlotPair {
        SlotPair {
            agent: agent.to_owned(),
            pool: pool.map(str::to_owned),
        }
    }

    fn limits(per_agent: u32, per_pool: u32) -> SlotLimits {
        SlotLimits::new(per_agent, per_pool).expect("limits of at least one")
    }

    fn registered(
        broker: &mut PermitBroker,
        fold: &TopologyFold,
        id: &InvocationId,
        slots: Option<SlotPair>,
    ) -> Admission {
        broker
            .register(&Standing::of(fold, id), id, slots)
            .unwrap_or_else(|error| panic!("`{id}` registers: {error}"))
    }

    fn completed(broker: &mut PermitBroker, id: &InvocationId) -> Vec<InvocationId> {
        broker
            .complete(id)
            .unwrap_or_else(|error| panic!("`{id}` completes: {error}"))
    }

    fn census_prefix(len_from_end: usize) -> TopologyFold {
        census_prefix_at_width(len_from_end, None)
    }

    fn census_prefix_at_width(len_from_end: usize, width: Option<u32>) -> TopologyFold {
        let mut trace = crate::topology::census::tests::deferred_verification_trace();
        trace.truncate(trace.len().saturating_sub(len_from_end));
        if let (Some(width), Some(first)) = (width, trace.first_mut()) {
            if let TopologyEventBody::RunStarted { data } = &mut first.body {
                data.limits.max_parallel = width;
            }
        }
        TopologyFold::replay(crate::topology::census::tests::inputs(), &trace)
            .expect("a prefix of the census's deferred-verification trace replays")
    }

    fn total(broker: &PermitBroker, fold: &TopologyFold) -> (usize, usize) {
        let derived = Entitlements::of(fold);
        (
            derived.pipeline_held() + broker.reservations().pipeline_outstanding(),
            derived.merge_held() + broker.reservations().merge_outstanding(),
        )
    }

    // --- R1/R2: fold-derived entitlements ---------------------------------

    #[test]
    fn pipeline_entitlements_are_read_from_the_fold_and_never_stored() {
        let mut fold = started_at_width(3);
        assert_eq!(Entitlements::of(&fold).pipeline_held(), 0);
        assert_eq!(Entitlements::of(&fold).max_parallel(), 3);

        apply(&mut fold, &dispatch(ALEPH, 0));
        assert_eq!(
            Entitlements::of(&fold).pipeline_held(),
            1,
            "an open generation with no attempt holds the pipeline entitlement"
        );
        in_flight(&mut fold, BET, 0);
        assert_eq!(
            Entitlements::of(&fold).pipeline_held(),
            2,
            "an in-flight generation holds it"
        );
        retained_generation(&mut fold, GIMEL, 0);
        assert_eq!(
            Entitlements::of(&fold).pipeline_held(),
            2,
            "a retained-idle generation holds none"
        );
        assert_eq!(
            Entitlements::of(&fold).pipeline_held(),
            fold.pipeline_held(),
            "the broker reads the fold's own count"
        );
        assert_eq!(Entitlements::of(&fold).merge_held(), 0);

        let restarted = PermitBroker::for_run(&Entitlements::of(&fold));
        assert!(
            restarted.is_empty() && restarted.balances(),
            "a process starts with empty ledgers and reads its entitlements from the fold"
        );
        assert_eq!(
            restarted.invocations().slots().limits(),
            SlotLimits::defaulted(3),
            "the slot limits default to the recorded max_parallel"
        );
    }

    #[test]
    fn an_unresolved_transaction_holds_both_entitlements_and_a_queued_candidate_neither() {
        let queued = census_prefix(2);
        assert!(queued.transaction().is_none());
        assert_eq!(
            (
                Entitlements::of(&queued).pipeline_held(),
                Entitlements::of(&queued).merge_held()
            ),
            (0, 0),
            "a queued candidate holds no entitlement"
        );

        let verifying = census_prefix(1);
        assert!(verifying.transaction().is_some());
        assert_eq!(
            (
                Entitlements::of(&verifying).pipeline_held(),
                Entitlements::of(&verifying).merge_held()
            ),
            (1, 1),
            "a started verification holds {{pipeline, merge}}"
        );

        let deferred = census_prefix(0);
        assert!(deferred.transaction().is_none());
        assert_eq!(
            (
                Entitlements::of(&deferred).pipeline_held(),
                Entitlements::of(&deferred).merge_held()
            ),
            (0, 0),
            "the transaction terminal releases both"
        );
    }

    // --- R13: provisional reservations ------------------------------------

    #[test]
    fn a_reservation_is_taken_only_when_the_derived_count_plus_the_outstanding_permits() {
        let mut fold = started_at_width(2);
        let mut broker = PermitBroker::for_run(&Entitlements::of(&fold));

        broker
            .reserve(&Entitlements::of(&fold), ALEPH, ReservationKind::Dispatch)
            .expect("the first of two");
        broker
            .reserve(&Entitlements::of(&fold), BET, ReservationKind::Dispatch)
            .expect("several are outstanding at width above one, one per selected task");
        assert_eq!(broker.reservations().outstanding(), 2);

        let refused = broker
            .reserve(&Entitlements::of(&fold), GIMEL, ReservationKind::Dispatch)
            .expect_err("0 derived + 2 outstanding is 2 of 2");
        assert!(
            refused.to_string().contains("is never awaited"),
            "{refused}"
        );
        assert_eq!(
            broker.reservations().held(GIMEL),
            None,
            "a refusal takes nothing and waits for nothing"
        );
        assert!(
            broker
                .reserve(&Entitlements::of(&fold), ALEPH, ReservationKind::Dispatch)
                .is_err(),
            "one reservation per selected task"
        );

        apply(&mut fold, &dispatch(ALEPH, 0));
        broker
            .convert(ALEPH, ReservationKind::Dispatch)
            .expect("converted at task_dispatched");
        assert_eq!(
            total(&broker, &fold),
            (2, 0),
            "the conversion moves the count, not the total"
        );
        assert!(
            broker
                .reserve(&Entitlements::of(&fold), GIMEL, ReservationKind::Dispatch)
                .is_err(),
            "1 derived + 1 outstanding is still 2 of 2"
        );

        broker
            .cancel_reservation(BET, ReservationKind::Dispatch)
            .expect("a pre-append failure cancels it");
        broker
            .reserve(&Entitlements::of(&fold), GIMEL, ReservationKind::Dispatch)
            .expect("1 derived + 0 outstanding admits one more");
        broker
            .cancel_reservation(GIMEL, ReservationKind::Dispatch)
            .expect("cancelled");
        assert!(broker.balances());
        assert_eq!(
            (
                broker.reservations().converted(),
                broker.reservations().cancelled()
            ),
            (1, 2)
        );
    }

    #[test]
    fn the_merge_entitlement_admits_one_integration_reservation() {
        let queued = census_prefix_at_width(2, Some(3));
        let key = queued
            .queue()
            .and_then(|queue| queue.entries().first())
            .map(|entry| entry.candidate.key)
            .expect("the census trace queues one candidate");
        let other = TaskKey(key.0 + 1);
        let mut broker = PermitBroker::for_run(&Entitlements::of(&queued));
        broker
            .reserve(
                &Entitlements::of(&queued),
                key,
                ReservationKind::Integration,
            )
            .expect("{pipeline, merge} is free");
        assert_eq!(broker.reservations().entitlements_held(), 2);
        assert_eq!(total(&broker, &queued), (1, 1));
        let refused = broker
            .reserve(
                &Entitlements::of(&queued),
                other,
                ReservationKind::Integration,
            )
            .expect_err("the provisional pair holds the one merge entitlement");
        assert!(
            refused
                .to_string()
                .contains("the one merge entitlement is held"),
            "{refused}"
        );
        broker
            .reserve(&Entitlements::of(&queued), other, ReservationKind::Dispatch)
            .expect("pipeline entitlements remain at width 3");

        let verifying = census_prefix_at_width(1, Some(3));
        let mut late = PermitBroker::for_run(&Entitlements::of(&verifying));
        let refused = late
            .reserve(
                &Entitlements::of(&verifying),
                other,
                ReservationKind::Integration,
            )
            .expect_err("the fold's open transaction holds the one merge entitlement");
        assert!(
            refused
                .to_string()
                .contains("the one merge entitlement is held"),
            "{refused}"
        );
        late.reserve(
            &Entitlements::of(&verifying),
            other,
            ReservationKind::Dispatch,
        )
        .expect("while a pipeline entitlement remains");
    }

    #[test]
    fn each_kind_converts_at_its_first_append_and_the_total_never_moves() {
        let mut fold = started_at_width(1);
        let mut broker = PermitBroker::for_run(&Entitlements::of(&fold));
        broker
            .reserve(&Entitlements::of(&fold), ALEPH, ReservationKind::Dispatch)
            .expect("dispatch reserves {pipeline}");
        assert_eq!(total(&broker, &fold), (1, 0));
        apply(&mut fold, &dispatch(ALEPH, 0));
        broker
            .convert(ALEPH, ReservationKind::Dispatch)
            .expect("at task_dispatched");
        assert_eq!(
            total(&broker, &fold),
            (1, 0),
            "dispatch: provisional became derived"
        );

        let mut retrying = started_at_width(1);
        retained_generation(&mut retrying, ALEPH, 0);
        assert_eq!(
            Entitlements::of(&retrying).pipeline_held(),
            0,
            "retained idle holds none"
        );
        let mut broker = PermitBroker::for_run(&Entitlements::of(&retrying));
        let worktrees = crate::engine::topology::settle::tests::FixedVerify::passing();
        let mut hooks =
            crate::engine::topology::seams::HarnessTopologyHooks::new(std::sync::Arc::new(
                std::sync::Mutex::new(crate::topology::effects::HookHarness::new()),
            ));
        let outcome = crate::engine::topology::settle::retry(
            &retrying,
            broker.halves().0,
            &worktrees,
            <crate::engine::topology::seams::HarnessTopologyHooks as crate::engine::topology::seams::TopologyHooks>::effects(&mut hooks),
            &crate::engine::topology::settle::tests::retry_request(ALEPH, 0),
        )
        .expect("the retry reserves through the broker");
        let crate::engine::topology::settle::RetryOutcome::Start(started) = outcome else {
            panic!("a verified worktree starts the retry");
        };
        assert_eq!(
            total(&broker, &retrying),
            (1, 0),
            "retry re-entry takes {{pipeline}}"
        );
        apply(
            &mut retrying,
            &ev(TopologyEventBody::AttemptStarted { data: *started }),
        );
        broker
            .convert(ALEPH, ReservationKind::Retry)
            .expect("at attempt_started(retry)");
        assert_eq!(
            total(&broker, &retrying),
            (1, 0),
            "retry: provisional became derived"
        );

        let queued = census_prefix(2);
        let verifying = census_prefix(1);
        let open = verifying.transaction().expect("the verification started");
        let mut broker = PermitBroker::for_run(&Entitlements::of(&queued));
        broker
            .reserve(
                &Entitlements::of(&queued),
                open.candidate.key,
                ReservationKind::Integration,
            )
            .expect("integration reserves {pipeline, merge}");
        assert_eq!(total(&broker, &queued), (1, 1));
        broker
            .convert(open.candidate.key, ReservationKind::Integration)
            .expect("at merge_verification_started");
        assert_eq!(
            broker.reservations().entitlements_held(),
            0,
            "the pair converts in one operation, never one half"
        );
        assert_eq!(
            total(&broker, &verifying),
            (1, 1),
            "integration: provisional became derived"
        );
        assert_eq!(broker.reservations().converted(), 1);
        assert!(broker.balances());
    }

    #[test]
    fn the_fast_path_converts_its_pair_at_merge_prepared_and_releases_both_at_task_merged() {
        use crate::engine::topology::integrate::{
            ExactBase, IntegrationRequest, decide, prepare_fast, publish,
        };
        use crate::engine::topology::scaffold::{ALPHA, Run};

        let mut run = Run::started("broker-fast-pair");
        let candidate = run.queue_candidate(ALPHA);
        let request = IntegrationRequest::from_log(
            run.emitter.fold(),
            &run.emitter.durable_events(),
            &candidate,
        )
        .expect("request");
        let manager = run.fixture.manager.clone();

        let derived = Entitlements::of(run.emitter.fold());
        run.reservations
            .reserve(ALPHA, ReservationKind::Integration, &derived)
            .expect("the broker's check admits the pair");
        assert_eq!(
            (derived.pipeline_held(), derived.merge_held()),
            (0, 0),
            "a queued candidate holds nothing"
        );
        assert_eq!(run.reservations.entitlements_held(), 2);

        let decided = decide(&manager, &request).expect("the head reads");
        assert_eq!(decided.exact_base, ExactBase::Fast);
        let authorized =
            prepare_fast(&mut run, &request, decided.head).expect("merge_prepared(fast)");
        let prepared = Entitlements::of(run.emitter.fold());
        assert_eq!(
            (prepared.pipeline_held(), prepared.merge_held()),
            (1, 1),
            "both derived holdings are present before the CAS"
        );
        assert!(
            run.reservations.is_empty() && run.reservations.converted() == 1,
            "the provisional pair converted once, atomically, at merge_prepared(fast)"
        );

        publish(&mut run, &manager, authorized).expect("publish");
        let merged = Entitlements::of(run.emitter.fold());
        assert_eq!(
            (merged.pipeline_held(), merged.merge_held()),
            (0, 0),
            "task_merged releases both, once"
        );
        assert!(run.reservations.balances());
    }

    #[test]
    fn reservations_are_cancelled_on_a_pre_append_failure_run_end_shutdown_or_a_poisoned_fold() {
        let mut fold = started_at_width(3);
        let mut broker = PermitBroker::for_run(&Entitlements::of(&fold));
        broker
            .reserve(&Entitlements::of(&fold), ALEPH, ReservationKind::Dispatch)
            .expect("reserved");
        broker
            .cancel_reservation(ALEPH, ReservationKind::Dispatch)
            .expect("a pre-append failure cancels the one it names");
        assert!(broker.balances());

        for key in [ALEPH, BET, GIMEL] {
            broker
                .reserve(&Entitlements::of(&fold), key, ReservationKind::Dispatch)
                .expect("reserved");
        }
        let (reservations, _) = broker.halves();
        assert_eq!(
            reservations.cancel_all(),
            3,
            "run end cancels every outstanding reservation"
        );
        assert!(broker.balances());

        for key in [ALEPH, BET] {
            broker
                .reserve(&Entitlements::of(&fold), key, ReservationKind::Dispatch)
                .expect("reserved");
        }
        assert_eq!(broker.shut_down().reservations, 2, "shutdown cancels them");
        assert!(broker.balances());

        broker
            .reserve(&Entitlements::of(&fold), GIMEL, ReservationKind::Dispatch)
            .expect("reserved");
        fold.poison();
        assert!(
            broker.halves().0.cancel_any(),
            "the append-error protocol cancels what is held on a poisoned fold"
        );
        let refused = broker
            .reserve(&Entitlements::of(&fold), GIMEL, ReservationKind::Dispatch)
            .expect_err("a poisoned fold admits nothing");
        assert!(refused.to_string().contains("poisoned"), "{refused}");
        assert!(broker.balances(), "the ledger balances at process end");
        assert_eq!(
            broker.reservations().cancelled(),
            7,
            "one pre-append failure, three at run end, two at shutdown, one poisoned"
        );
    }

    // --- the acquisition order ---------------------------------------------

    #[test]
    fn a_slot_request_from_a_pipeline_holding_no_entitlement_is_refused() {
        let mut fold = started_at_width(3);
        let mut broker = PermitBroker::for_run(&Entitlements::of(&fold));
        let worker = of(ALEPH).worker();

        let refused = broker
            .register(
                &Standing::of(&fold, &worker),
                &worker,
                Some(pair(AGENT, None)),
            )
            .expect_err("no generation");
        assert!(
            refused
                .to_string()
                .contains("pipeline -> merge -> {agent, pool}"),
            "{refused}"
        );
        apply(&mut fold, &dispatch(ALEPH, 0));
        assert!(
            broker
                .register(
                    &Standing::of(&fold, &worker),
                    &worker,
                    Some(pair(AGENT, None))
                )
                .is_err(),
            "an open generation holds the entitlement but no attempt of it runs yet"
        );
        assert_eq!(
            broker.invocations().registered(),
            0,
            "the refused requests registered nothing"
        );
        let started = crate::engine::topology::settle::tests::attempt_started(&fold, ALEPH, 0, 1);
        apply(&mut fold, &started);
        assert_eq!(Standing::of(&fold, &worker).holds(), Holds::Pipeline);
        assert_eq!(
            registered(&mut broker, &fold, &worker, Some(pair(AGENT, None))),
            Admission::Granted
        );
        let stale = AttemptIdentities::new(ALEPH, GenerationId(0), AttemptNumber(2)).worker();
        assert_eq!(
            Standing::of(&fold, &stale).holds(),
            Holds::Nothing,
            "an attempt the fold does not have in flight is stale"
        );
        completed(&mut broker, &worker);

        let verifying = census_prefix(1);
        let open = verifying.transaction().expect("started").sequence;
        let review = SequenceIdentities::new(open).review_pass(0, 0);
        assert_eq!(
            Standing::of(&verifying, &review).holds(),
            Holds::PipelineAndMerge
        );
        let other = SequenceIdentities::new(SequenceId(open.0 + 1)).review_pass(0, 0);
        assert_eq!(Standing::of(&verifying, &other).holds(), Holds::Nothing);
        assert_eq!(
            Standing::of(&census_prefix(2), &review).holds(),
            Holds::Nothing,
            "no verification has started"
        );

        let probe = PreflightIdentities::agent(AGENT, 0).expect("a probe");
        assert_eq!(
            Standing::of(&started_at_width(1), &probe).holds(),
            Holds::Preflight
        );
        assert!(
            Standing::preflight().admits(&worker).is_err(),
            "the pre-flight's standing admits probes only"
        );

        let mut poisoned = three_in_flight();
        poisoned.poison();
        assert_eq!(
            Standing::of(&poisoned, &of(BET).worker()).holds(),
            Holds::Nothing
        );

        let alephs = Standing::of(&fold, &of(ALEPH).review_pass(0, 0));
        let refused = broker
            .halves()
            .1
            .register_slotted(&of(BET).worker(), pair(AGENT, None), &alephs)
            .expect_err("a standing read for one pipeline admits no other's invocation");
        assert!(
            refused.to_string().contains("standing of another pipeline"),
            "{refused}"
        );
        assert!(broker.balances());
    }

    // --- acceptance item 3: the slot tests ----------------------------------

    #[test]
    fn same_agent_and_pool_with_opposing_limits_serialize_on_the_binding_limit() {
        let fold = three_in_flight();
        for (slot_limits, binding) in [(limits(1, 2), "agent"), (limits(2, 1), "pool")] {
            let mut broker = PermitBroker::new(slot_limits);
            let (first, second) = (of(ALEPH).worker(), of(BET).worker());
            assert_eq!(
                registered(&mut broker, &fold, &first, Some(pair(AGENT, Some(POOL)))),
                Admission::Granted
            );
            assert_eq!(
                registered(&mut broker, &fold, &second, Some(pair(AGENT, Some(POOL)))),
                Admission::Pending,
                "the {binding} limit binds"
            );
            assert_eq!(completed(&mut broker, &first), vec![second.clone()]);
            completed(&mut broker, &second);
            assert!(broker.balances());
        }
        let mut broker = PermitBroker::new(limits(2, 2));
        registered(
            &mut broker,
            &fold,
            &of(ALEPH).worker(),
            Some(pair(AGENT, Some(POOL))),
        );
        assert_eq!(
            registered(
                &mut broker,
                &fold,
                &of(BET).worker(),
                Some(pair(AGENT, Some(POOL)))
            ),
            Admission::Granted,
            "the control: with neither limit binding the two run together"
        );
    }

    #[test]
    fn two_agents_with_their_own_pools_run_in_parallel() {
        let fold = three_in_flight();
        let mut broker = PermitBroker::new(limits(1, 1));
        let (first, second) = (of(ALEPH).worker(), of(BET).worker());
        assert_eq!(
            registered(&mut broker, &fold, &first, Some(pair(AGENT, Some(POOL)))),
            Admission::Granted
        );
        assert_eq!(
            registered(
                &mut broker,
                &fold,
                &second,
                Some(pair(OTHER_AGENT, Some(OTHER_POOL)))
            ),
            Admission::Granted
        );
        assert_eq!(broker.invocations().slots().holders().len(), 2);
        completed(&mut broker, &first);
        completed(&mut broker, &second);
        assert!(broker.balances());
    }

    #[test]
    fn an_agent_without_a_pool_takes_its_agent_slot_only() {
        let fold = three_in_flight();
        let mut broker = PermitBroker::new(limits(1, 1));
        let pooled = of(ALEPH).worker();
        let unpooled = of(BET).worker();
        registered(
            &mut broker,
            &fold,
            &pooled,
            Some(pair(OTHER_AGENT, Some(POOL))),
        );
        assert_eq!(
            registered(&mut broker, &fold, &unpooled, Some(pair(AGENT, None))),
            Admission::Granted,
            "the pool at its limit does not bind an agent that has no pool"
        );
        let slots = broker.invocations().slots();
        assert_eq!(
            (slots.held_by_agent(AGENT), slots.held_by_pool(POOL)),
            (1, 1),
            "the unpooled agent took one agent slot and no pool slot"
        );
        completed(&mut broker, &pooled);
        completed(&mut broker, &unpooled);
        assert!(broker.balances());
    }

    #[test]
    fn agent_probes_acquire_and_release_their_pair() {
        let fold = started_at_width(1);
        let mut broker = PermitBroker::for_run(&Entitlements::of(&fold));
        let first = PreflightIdentities::agent(AGENT, 0).expect("a probe");
        let second = PreflightIdentities::agent(AGENT, 1).expect("a probe");
        assert_eq!(
            registered(&mut broker, &fold, &first, Some(pair(AGENT, None))),
            Admission::Granted
        );
        assert_eq!(
            broker.invocations().slots().pair_of(&first),
            Some(&pair(AGENT, None))
        );
        assert_eq!(
            registered(&mut broker, &fold, &second, Some(pair(AGENT, None))),
            Admission::Pending,
            "a probe holds its agent slot like any other agent CLI invocation"
        );
        assert_eq!(completed(&mut broker, &first), vec![second.clone()]);
        completed(&mut broker, &second);
        assert!(broker.balances());
        assert_eq!(broker.invocations().slots().released(), 2);
    }

    #[test]
    fn gate_and_shell_probe_invocations_register_without_slots() {
        let fold = three_in_flight();
        let mut broker = PermitBroker::new(limits(1, 1));
        let holder = of(ALEPH).worker();
        registered(&mut broker, &fold, &holder, Some(pair(AGENT, Some(POOL))));
        let gate = of(ALEPH).gate(0, 0);
        let shell = PreflightIdentities::shell(0).expect("the shell probe");
        for id in [&gate, &shell] {
            assert_eq!(
                registered(&mut broker, &fold, id, None),
                Admission::Runnable,
                "`{id}` is runnable at once while every slot is held"
            );
            assert!(!broker.invocations().slots().holds(id));
        }
        assert!(
            broker
                .register(
                    &Standing::of(&fold, &of(BET).gate(0, 0)),
                    &of(BET).gate(0, 0),
                    Some(pair(AGENT, None))
                )
                .is_err(),
            "a gate offered a pair refuses it"
        );
        assert!(
            broker
                .register(
                    &Standing::of(&fold, &of(BET).worker()),
                    &of(BET).worker(),
                    None
                )
                .is_err(),
            "a worker offered no pair refuses to register"
        );
        for id in [&gate, &shell, &holder] {
            completed(&mut broker, id);
        }
        assert_eq!(
            broker.invocations().slots().granted(),
            1,
            "only the worker took a pair"
        );
        assert!(broker.balances());
    }

    #[test]
    fn broker_only_two_agents_sharing_one_pool_serialize_on_the_pool() {
        let fold = three_in_flight();
        let mut broker = PermitBroker::new(limits(2, 1));
        let (first, second) = (of(ALEPH).worker(), of(BET).worker());
        registered(&mut broker, &fold, &first, Some(pair(AGENT, Some(POOL))));
        assert_eq!(
            registered(
                &mut broker,
                &fold,
                &second,
                Some(pair(OTHER_AGENT, Some(POOL)))
            ),
            Admission::Pending,
            "broker-only: two agents in one pool is unreachable under the one-agent-per-pool model"
        );
        assert_eq!(completed(&mut broker, &first), vec![second.clone()]);
        completed(&mut broker, &second);
        assert!(broker.balances());
    }

    #[test]
    fn broker_only_one_agent_in_two_pools_serializes_on_the_agent() {
        let fold = three_in_flight();
        let mut broker = PermitBroker::new(limits(1, 2));
        let (first, second) = (of(ALEPH).worker(), of(BET).worker());
        registered(&mut broker, &fold, &first, Some(pair(AGENT, Some(POOL))));
        assert_eq!(
            registered(
                &mut broker,
                &fold,
                &second,
                Some(pair(AGENT, Some(OTHER_POOL)))
            ),
            Admission::Pending,
            "broker-only: an agent is in one pool under the current model"
        );
        assert_eq!(completed(&mut broker, &first), vec![second.clone()]);
        completed(&mut broker, &second);
        assert!(broker.balances());
    }

    #[test]
    fn a_reviewer_and_a_worker_of_one_agent_never_hold_its_slot_at_once() {
        let fold = three_in_flight();
        let mut broker = PermitBroker::new(limits(1, 1));
        let worker = of(ALEPH).worker();
        let reviewer = of(BET).review_pass(0, 0);
        registered(&mut broker, &fold, &worker, Some(pair(AGENT, None)));
        assert_eq!(
            registered(&mut broker, &fold, &reviewer, Some(pair(AGENT, None))),
            Admission::Pending,
            "at max_per_agent = 1 the reviewer waits for the worker's agent slot"
        );
        assert_eq!(broker.invocations().slots().held_by_agent(AGENT), 1);
        assert_eq!(completed(&mut broker, &worker), vec![reviewer.clone()]);
        assert_eq!(broker.invocations().slots().held_by_agent(AGENT), 1);
        completed(&mut broker, &reviewer);
        assert!(broker.balances());
    }

    // --- ST-02 / ST-05 ------------------------------------------------------

    #[test]
    fn duplicate_operations_are_ignored_and_counted_and_nothing_is_released_twice() {
        let fold = three_in_flight();
        let mut broker = PermitBroker::new(limits(1, 1));
        let worker = of(ALEPH).worker();
        let waiting = of(BET).worker();
        registered(&mut broker, &fold, &worker, Some(pair(AGENT, None)));
        registered(&mut broker, &fold, &waiting, Some(pair(AGENT, None)));

        assert_eq!(completed(&mut broker, &worker), vec![waiting.clone()]);
        for _ in 0..2 {
            assert_eq!(completed(&mut broker, &worker), Vec::new());
            assert_eq!(broker.cancel(&worker).expect("a late cancel"), Vec::new());
        }
        assert!(
            broker.invocations().slots().holds(&waiting),
            "the duplicates released nothing of the invocation granted after them"
        );
        assert_eq!(broker.invocations().duplicates(), 4);

        assert!(
            broker
                .convert(TaskKey(9), ReservationKind::Dispatch)
                .is_err(),
            "a reservation never taken is refused, not counted"
        );
        let mut fresh = started_at_width(3);
        broker
            .reserve(&Entitlements::of(&fresh), ALEPH, ReservationKind::Dispatch)
            .expect("reserved");
        apply(&mut fresh, &dispatch(ALEPH, 0));
        broker
            .convert(ALEPH, ReservationKind::Dispatch)
            .expect("converted at its append");
        broker
            .convert(ALEPH, ReservationKind::Dispatch)
            .expect("a duplicate conversion is ignored");
        broker
            .cancel_reservation(ALEPH, ReservationKind::Dispatch)
            .expect("and a stale cancel");
        assert_eq!(
            (
                broker.reservations().converted(),
                broker.reservations().cancelled(),
                broker.reservations().duplicates()
            ),
            (1, 0, 2),
            "settled once; the rest counted"
        );
        assert_eq!(
            total(&broker, &fresh),
            (1, 0),
            "and the derived count holds it once"
        );

        completed(&mut broker, &waiting);
        assert!(broker.balances());
        assert_eq!(broker.duplicates(), 6);
    }

    // --- seeded adversarial orders -------------------------------------------

    struct Seeded(u64);

    impl Seeded {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, bound: usize) -> usize {
            let bound = u64::try_from(bound.max(1)).expect("a small bound");
            usize::try_from(self.next() % bound).expect("below a usize bound")
        }

        fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
            let index = self.below(items.len());
            items.get(index).expect("an index below the length")
        }
    }

    const LOAD_AGENTS: [&str; 3] = ["claude", "codex", "copilot"];
    const LOAD_POOLS: [Option<&str>; 3] = [Some("anthropic"), Some("openai"), None];

    struct Trial {
        ledger: InvocationLedger,
        limits: SlotLimits,
        pairs: BTreeMap<InvocationId, SlotPair>,
        arrival: BTreeMap<InvocationId, usize>,
        granted_at: BTreeMap<InvocationId, u64>,
        grants: BTreeMap<InvocationId, u32>,
        withdrawn: BTreeSet<InvocationId>,
        settled: Vec<InvocationId>,
        head: Option<InvocationId>,
        became_head_at: u64,
        tick: u64,
        injected: u32,
    }

    impl Trial {
        fn new(limits: SlotLimits) -> Self {
            Self {
                ledger: InvocationLedger::with_limits(limits),
                limits,
                pairs: BTreeMap::new(),
                arrival: BTreeMap::new(),
                granted_at: BTreeMap::new(),
                grants: BTreeMap::new(),
                withdrawn: BTreeSet::new(),
                settled: Vec::new(),
                head: None,
                became_head_at: 0,
                tick: 0,
                injected: 0,
            }
        }

        fn request(&mut self, pair: SlotPair) {
            let ordinal = u32::try_from(self.arrival.len()).expect("a small trial");
            let id = PreflightIdentities::agent("load", ordinal).expect("a probe identity");
            self.arrival.insert(id.clone(), self.arrival.len());
            self.pairs.insert(id.clone(), pair.clone());
            let admission = self
                .ledger
                .register_slotted(&id, pair, &Standing::preflight())
                .expect("a fresh identity registers");
            let granted = if admission == Admission::Granted {
                vec![id]
            } else {
                Vec::new()
            };
            self.record(&granted);
        }

        fn complete_one(&mut self, seeded: &mut Seeded) -> bool {
            let holders: Vec<InvocationId> =
                self.ledger.slots().holders().into_iter().cloned().collect();
            if holders.is_empty() {
                return false;
            }
            let id = seeded.pick(&holders).clone();
            let granted = self.ledger.complete(&id).expect("a holder completes");
            self.settled.push(id);
            self.record(&granted);
            true
        }

        fn cancel_one(&mut self, seeded: &mut Seeded) {
            let pending: Vec<InvocationId> =
                self.ledger.slots().pending().into_iter().cloned().collect();
            if pending.is_empty() {
                return;
            }
            let id = seeded.pick(&pending).clone();
            let granted = self
                .ledger
                .cancel(&id)
                .expect("a waiting request is withdrawn");
            self.withdrawn.insert(id.clone());
            self.settled.push(id);
            self.record(&granted);
        }

        fn duplicate_one(&mut self, seeded: &mut Seeded) {
            if self.settled.is_empty() {
                return;
            }
            let id = seeded.pick(&self.settled).clone();
            let holders = self.ledger.slots().holders().len();
            let pending = self.ledger.slots().pending().len();
            let granted = if seeded.below(2) == 0 {
                self.ledger.complete(&id)
            } else {
                self.ledger.cancel(&id)
            }
            .expect("a duplicate settlement is not an error");
            self.injected += 1;
            assert!(
                granted.is_empty(),
                "a duplicate of `{id}` granted {granted:?}"
            );
            assert_eq!(
                (
                    self.ledger.slots().holders().len(),
                    self.ledger.slots().pending().len()
                ),
                (holders, pending),
                "a duplicate of `{id}` moved the table"
            );
        }

        fn record(&mut self, granted: &[InvocationId]) {
            let head_after = self.ledger.slots().head().cloned();
            let arrival_of_head = head_after
                .as_ref()
                .and_then(|head| self.arrival.get(head).copied());
            let (fronts, backfills): (Vec<&InvocationId>, Vec<&InvocationId>) =
                granted.iter().partition(|id| {
                    arrival_of_head.is_none_or(|head| {
                        self.arrival.get(*id).copied().unwrap_or(usize::MAX) < head
                    })
                });
            for id in fronts {
                self.grant(id);
            }
            if head_after != self.head {
                self.tick += 1;
                self.became_head_at = self.tick;
                self.head = head_after;
            }
            for id in backfills {
                self.grant(id);
            }
        }

        fn grant(&mut self, id: &InvocationId) {
            self.tick += 1;
            self.granted_at.insert(id.clone(), self.tick);
            *self.grants.entry(id.clone()).or_insert(0) += 1;
        }

        fn check(&self, seed: u64) {
            let slots = self.ledger.slots();
            for agent in LOAD_AGENTS {
                assert!(
                    slots.held_by_agent(agent) <= self.limits.per_agent(),
                    "seed {seed}: agent `{agent}` over its limit"
                );
            }
            for pool in LOAD_POOLS.iter().flatten() {
                assert!(
                    slots.held_by_pool(pool) <= self.limits.per_pool(),
                    "seed {seed}: pool `{pool}` over its limit"
                );
            }
            assert_eq!(
                u64::from(slots.granted()) - u64::from(slots.released()),
                u64::try_from(slots.holders().len()).expect("a small table"),
                "seed {seed}: the grant and release counts disagree with the holders"
            );
            let Some(head) = slots.head() else {
                return;
            };
            let pair = self.pairs.get(head).expect("the head was issued");
            let agent_blocked = slots.held_by_agent(&pair.agent) >= self.limits.per_agent();
            let pool_blocked = pair
                .pool
                .as_deref()
                .is_some_and(|pool| slots.held_by_pool(pool) >= self.limits.per_pool());
            assert!(
                agent_blocked || pool_blocked,
                "seed {seed}: the head `{head}` waits while its pair fits"
            );
            for holder in slots.holders() {
                let held = slots.pair_of(holder).expect("a holder's pair");
                let blocks = (agent_blocked && held.agent == pair.agent)
                    || (pool_blocked && held.pool.is_some() && held.pool == pair.pool);
                if blocks {
                    let granted_at = self.granted_at.get(holder).copied().unwrap_or(u64::MAX);
                    assert!(
                        granted_at < self.became_head_at,
                        "seed {seed}: `{holder}` took a slot the head `{head}` reserved"
                    );
                }
            }
        }
    }

    #[test]
    fn seeded_adversarial_orders_grant_every_request_in_order_and_exceed_no_limit() {
        for seed in 0..256_u64 {
            let mut seeded = Seeded(seed);
            let per_agent = 1 + u32::try_from(seeded.below(3)).expect("small");
            let per_pool = 1 + u32::try_from(seeded.below(3)).expect("small");
            let mut trial = Trial::new(limits(per_agent, per_pool));
            for _ in 0..64 {
                match seeded.below(5) {
                    0..=2 => {
                        let agent = *seeded.pick(&LOAD_AGENTS);
                        let pool = *seeded.pick(&LOAD_POOLS);
                        trial.request(pair(agent, pool));
                    }
                    3 => {
                        trial.complete_one(&mut seeded);
                    }
                    _ => trial.cancel_one(&mut seeded),
                }
                trial.check(seed);
            }
            while trial.complete_one(&mut seeded) {
                trial.check(seed);
            }
            assert!(
                trial.ledger.slots().pending().is_empty(),
                "seed {seed}: a request is still waiting with nothing held"
            );
            for id in trial.arrival.keys() {
                let expected = u32::from(!trial.withdrawn.contains(id));
                assert_eq!(
                    trial.grants.get(id).copied().unwrap_or(0),
                    expected,
                    "seed {seed}: `{id}` was granted a number of times other than {expected}"
                );
            }
            assert!(
                trial.ledger.balances(),
                "seed {seed}: the ledger did not balance"
            );
        }
    }

    #[test]
    fn injected_duplicate_settlements_release_nothing_twice_and_no_count_goes_negative() {
        for seed in 0..128_u64 {
            let mut seeded = Seeded(seed ^ 0x5EED);
            let mut trial = Trial::new(limits(2, 1));
            for _ in 0..64 {
                match seeded.below(6) {
                    0 | 1 => {
                        let agent = *seeded.pick(&LOAD_AGENTS);
                        let pool = *seeded.pick(&LOAD_POOLS);
                        trial.request(pair(agent, pool));
                    }
                    2 => {
                        trial.complete_one(&mut seeded);
                    }
                    3 => trial.cancel_one(&mut seeded),
                    _ => trial.duplicate_one(&mut seeded),
                }
                trial.check(seed);
            }
            while trial.complete_one(&mut seeded) {
                trial.check(seed);
            }
            assert_eq!(
                trial.ledger.duplicates(),
                trial.injected,
                "seed {seed}: every injected duplicate was counted, and nothing else"
            );
            assert_eq!(
                trial.ledger.slots().released(),
                trial.ledger.slots().granted(),
                "seed {seed}: a pair was released other than once"
            );
            assert!(
                trial.ledger.balances(),
                "seed {seed}: the ledger did not balance"
            );
        }

        for seed in 0..128_u64 {
            let mut seeded = Seeded(seed ^ 0xD0_0B1E);
            let fold = started_at_width(3);
            let mut reservations = Reservations::new();
            let mut settled: Vec<TaskKey> = Vec::new();
            let mut injected = 0_u32;
            for _ in 0..48 {
                let key = *seeded.pick(&[ALEPH, BET, GIMEL]);
                match seeded.below(4) {
                    0 => {
                        let full = reservations.outstanding() == 3;
                        let holding = reservations.held(key).is_some();
                        let taken = reservations
                            .reserve(key, ReservationKind::Dispatch, &Entitlements::of(&fold))
                            .is_ok();
                        assert_eq!(
                            taken,
                            !full && !holding,
                            "seed {seed}: a reservation is refused exactly when the width is \
                             full or the task already holds one"
                        );
                    }
                    1 | 2 if reservations.held(key).is_some() => {
                        if seeded.below(2) == 0 {
                            reservations.convert(key, ReservationKind::Dispatch)
                        } else {
                            reservations.cancel(key, ReservationKind::Dispatch)
                        }
                        .expect("an outstanding reservation settles");
                        settled.push(key);
                    }
                    _ if !settled.is_empty()
                        && reservations.held(key).is_none()
                        && settled.contains(&key) =>
                    {
                        let before = reservations.outstanding();
                        reservations
                            .cancel(key, ReservationKind::Dispatch)
                            .expect("a stale cancel is ignored");
                        injected += 1;
                        assert_eq!(
                            reservations.outstanding(),
                            before,
                            "seed {seed}: released twice"
                        );
                    }
                    _ => {}
                }
                assert!(
                    reservations.outstanding() <= 3,
                    "seed {seed}: more reservations than max_parallel admits"
                );
                assert_eq!(
                    u64::from(reservations.taken()),
                    u64::from(reservations.converted())
                        + u64::from(reservations.cancelled())
                        + u64::try_from(reservations.outstanding()).expect("small"),
                    "seed {seed}: every reservation taken is outstanding or settled once"
                );
            }
            assert_eq!(reservations.duplicates(), injected, "seed {seed}");
            reservations.cancel_all();
            assert!(
                reservations.balances(),
                "seed {seed}: the provisional ledger did not balance"
            );
        }
    }
}
