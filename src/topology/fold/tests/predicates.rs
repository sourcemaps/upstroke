use super::*;

#[test]
fn a_poisoned_fold_offers_no_continuation_and_no_integration_candidate() {
    let mut fold = wide_started(3);
    queue_candidate(&mut fold, MID, 0);
    apply(&mut fold, &dispatch(BETA, 0, &sha("base")));

    assert_eq!(
        fold.eligible_continuation(BETA),
        Some(GenerationId(0)),
        "`beta` holds a dispatched generation whose first attempt has not started"
    );
    assert_eq!(
        fold.eligible_integration_candidate().map(|open| open.key),
        Some(MID),
        "`mid`'s candidate is queued, eligible, and the slot is free"
    );

    fold.poison();

    assert_eq!(
        fold.eligible_continuation(BETA),
        None,
        "a poisoned fold offered a continuation to select"
    );
    assert!(
        fold.eligible_integration_candidate().is_none(),
        "a poisoned fold offered an integration candidate to select"
    );
}

#[test]
fn open_no_attempt_names_the_open_generation_and_no_other() {
    let mut fold = wide_started(3);
    apply(&mut fold, &dispatch(BETA, 0, &sha("base")));
    retained_generation(&mut fold, ZETA, 0);

    assert_eq!(
        fold.open_no_attempt(BETA),
        Some(GenerationId(0)),
        "`beta` was dispatched and no attempt has started in its generation"
    );
    assert_eq!(
        fold.open_no_attempt(ZETA),
        None,
        "`zeta`'s open generation is retained idle; recovery recreates no worktree for it"
    );
    assert_eq!(
        fold.open_no_attempt(ALPHA),
        None,
        "`alpha` holds no generation at all"
    );
}

fn run_mut(fold: &mut TopologyFold) -> &mut RunState {
    fold.run.as_mut().expect("the fixture's run has started")
}

#[test]
fn an_exhausted_integration_sequence_opens_no_transaction() {
    let mut fold = wide_started(3);
    queue_candidate(&mut fold, MID, 0);
    let publish = |sequence| fast_publication(MID, 0, sequence, &sha("base"), vec![MID]);

    run_mut(&mut fold).next_sequence = u32::MAX - 1;
    accepts(&fold, &publish(u32::MAX - 1));

    run_mut(&mut fold).next_sequence = u32::MAX;
    let before = fold.run.clone();
    let last = publish(u32::MAX);
    let FoldError::InconsistentRecord { kind, detail } = refuse(&fold, &last) else {
        panic!("an exhausted sequence counter must return a contextual record refusal");
    };
    assert_eq!(kind, last.body.kind());
    assert!(
        detail.contains("exhausted its integration sequence numbers"),
        "{detail}"
    );
    assert_eq!(fold.run, before, "the refused transaction moved the fold");
}

#[test]
fn an_exhausted_epoch_admits_no_resume() {
    let mut fold = wide_started(3);
    let resumed = resume(container_runner());

    run_mut(&mut fold).epoch = Epoch(u32::MAX - 1);
    accepts(&fold, &resumed);

    run_mut(&mut fold).epoch = Epoch(u32::MAX);
    let before = fold.run.clone();
    let FoldError::InconsistentRecord { kind, detail } = refuse(&fold, &resumed) else {
        panic!("an exhausted epoch must return a contextual record refusal");
    };
    assert_eq!(kind, resumed.body.kind());
    assert!(detail.contains("exhausted its epoch numbers"), "{detail}");
    assert_eq!(fold.run, before, "the refused resume moved the fold");
}
