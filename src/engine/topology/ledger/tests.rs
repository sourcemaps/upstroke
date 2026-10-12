use super::*;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::engine::topology::candidate::run_namespace;
use crate::engine::topology::finalize::CleanupStep;
use crate::events::{BudgetKind, RunOutcome};
use crate::ir::{QuestionId, QuestionKind};
use crate::topology::effects::{
    EffectSiteId, HookPhase, Injection, RefSite, SnapshotSite, WorktreeSite,
};
use crate::topology::events::{
    BudgetExceeded4, FrozenQuestion, IncarnationId, QuestionRaised4, RunResumed4, RunStarted4,
};
use crate::workspace_manager::fixture::{Fixture, RUN_ID, git, git_os, remove_file, write_file};
use crate::workspace_manager::{
    EffectHooks, NoHooks, Slot, SnapshotName, WorkspaceManager, WriterProof,
};

/// `resource_accounting.completeness_rule`: one row per resource, every row
/// in every outcome's equation exactly once, and every row in exactly one
/// enforcement domain — the shape ST-09 iterates.
#[test]
fn every_outcome_equation_names_every_row_once() {
    for outcome in Outcome::ALL {
        let rows: Vec<Row> = equation(outcome)
            .iter()
            .map(|expectation| expectation.row)
            .collect();
        assert_eq!(rows, Row::ALL.to_vec(), "{outcome:?}");
    }
    let mut by_domain = std::collections::BTreeMap::<String, Vec<Row>>::new();
    for row in Row::ALL {
        by_domain
            .entry(format!("{:?}", row.domain()))
            .or_default()
            .push(row);
        assert!(!row.resource().is_empty());
    }
    assert_eq!(
        by_domain
            .iter()
            .map(|(domain, rows)| (domain.as_str(), rows.len()))
            .collect::<Vec<_>>(),
        vec![
            ("ExternalPhysical", 12),
            ("LogicalFoldBroker", 12),
            ("OperatorOwned", 1),
            ("ProcessLocalOs", 3),
        ],
        "the packet's four domains, twelve, twelve, one and three rows"
    );
}

/// `Requirement::admits` reads a before/after pair the way each word says.
#[test]
fn requirements_read_before_and_after_as_named() {
    use Fact::{Absent, Balanced, Held, Present, Unbalanced, Zero};
    assert!(Requirement::Zero.admits(Held(2), Zero));
    assert!(Requirement::Zero.admits(Held(2), Absent));
    assert!(!Requirement::Zero.admits(Zero, Held(1)));
    assert!(Requirement::Present.admits(Absent, Present(1)));
    assert!(!Requirement::Present.admits(Present(1), Absent));
    assert!(Requirement::Retained.admits(Present(2), Present(2)));
    assert!(Requirement::Retained.admits(Absent, Zero));
    assert!(!Requirement::Retained.admits(Present(2), Present(1)));
    assert!(!Requirement::Retained.admits(Present(1), Absent));
    assert!(Requirement::Balanced.admits(Unbalanced, Balanced));
    assert!(!Requirement::Balanced.admits(Balanced, Unbalanced));
    assert!(Requirement::Monotone.admits(Present(1), Present(3)));
    assert!(!Requirement::Monotone.admits(Present(3), Present(1)));
    assert!(Requirement::Any.admits(Unbalanced, Unbalanced));
}

/// The rows whose `resource_accounting` cell names several facts carry one
/// requirement per fact, and `check` reports a disagreement on the named
/// part: a report that went missing at Complete is R21's `report` part, not
/// a row that still reads Present because the other outputs are there. A-12
/// adds the late parts: `late_instances` on R9, R10 and R24 and
/// `late_referenced_objects` on R27, in every ended outcome.
#[test]
fn a_rows_named_facts_are_each_consumed_by_its_verdict() {
    let named: Vec<(Row, Vec<&str>)> = equation(Outcome::Complete)
        .iter()
        .filter(|expectation| !expectation.parts.is_empty())
        .map(|expectation| {
            (
                expectation.row,
                expectation.parts.iter().map(|(name, _, _)| *name).collect(),
            )
        })
        .collect();
    assert_eq!(
        named,
        vec![
            (Row::R9, vec!["late_instances"]),
            (Row::R10, vec!["late_instances"]),
            (
                Row::R14,
                vec![
                    "merge_sequence",
                    "task_keys",
                    "display_ids",
                    "generations",
                    "attempts",
                    "lineage_indexes",
                    "repair_budget_units",
                    "verification_defers",
                    "override_slots",
                ]
            ),
            (
                Row::R21,
                vec![
                    "integration_ref",
                    "events_jsonl",
                    "plan_normalized",
                    "report",
                    "question_payloads",
                    "answer_files",
                    "partial_files",
                    "marker",
                    "run_lock_file",
                    "cleanup_lock_file",
                    "owner_record",
                    "commit_record",
                    "private_artifacts",
                ]
            ),
            (Row::R24, vec!["late_instances"]),
            (
                Row::R27,
                vec![
                    "released_objects_missing",
                    "store_objects",
                    "store_objects_missing",
                    "unreachable_objects",
                    "late_referenced_objects",
                ]
            ),
        ],
        "the three rows the packet's cells name several facts for, every fact named, and the \
         late parts A-12 adds to R9, R10, R24 and R27"
    );
    for outcome in Outcome::ALL {
        let report = equation(outcome)
            .into_iter()
            .find(|expectation| expectation.row == Row::R21)
            .and_then(|expectation| {
                expectation
                    .parts
                    .iter()
                    .find(|(name, _, _)| *name == "report")
                    .map(|(_, _, holds)| *holds)
            })
            .expect("R21 names the report");
        assert_eq!(
            report,
            if outcome == Outcome::NoRunFinished {
                Requirement::Any
            } else {
                Requirement::Present
            },
            "{outcome:?}: the report is present at every run end and regenerated on resume"
        );
    }

    // A ledger whose R21 row reads Present while its report part reads Absent.
    let observation = |parts: Vec<Part>| Observation {
        row: Row::R21,
        fact: Fact::Present(1),
        parts,
        evidence: "synthetic".to_owned(),
    };
    let whole = |report: bool| {
        let mut parts = vec![
            Part {
                name: "integration_ref",
                fact: Fact::Present(1),
            },
            Part {
                name: "events_jsonl",
                fact: Fact::Present(1),
            },
            Part {
                name: "plan_normalized",
                fact: Fact::Present(1),
            },
            Part {
                name: "report",
                fact: if report {
                    Fact::Present(1)
                } else {
                    Fact::Absent
                },
            },
            Part {
                name: "question_payloads",
                fact: Fact::Absent,
            },
            Part {
                name: "answer_files",
                fact: Fact::Absent,
            },
            Part {
                name: "partial_files",
                fact: Fact::Absent,
            },
            Part {
                name: "marker",
                fact: Fact::Absent,
            },
            Part {
                name: "run_lock_file",
                fact: Fact::Present(1),
            },
            Part {
                name: "cleanup_lock_file",
                fact: Fact::Present(1),
            },
            Part {
                name: "owner_record",
                fact: Fact::Present(1),
            },
            Part {
                name: "commit_record",
                fact: Fact::Present(1),
            },
        ];
        parts.push(Part {
            name: "private_artifacts",
            fact: Fact::Absent,
        });
        Ledger {
            observations: vec![observation(parts)],
            at_sweep_inventory: false,
        }
    };
    let before = whole(true);
    let after = whole(false);
    let disagreements: Vec<Disagreement> = check(&before, &after, Outcome::Complete)
        .into_iter()
        .filter(|disagreement| disagreement.row == Row::R21)
        .collect();
    assert_eq!(disagreements.len(), 1, "{disagreements:#?}");
    assert_eq!(disagreements[0].part, Some("report"));
    assert_eq!(disagreements[0].expected, Requirement::Present);
    assert_eq!(disagreements[0].after, Fact::Absent);
    assert!(
        check(&before, &before, Outcome::Complete)
            .iter()
            .all(|disagreement| disagreement.row != Row::R21),
        "with the report present the row and every part hold"
    );
    let record = record(&before, &after, Outcome::Complete);
    let row = record
        .rows
        .iter()
        .find(|row| row.row == Row::R21)
        .expect("R21 is recorded");
    assert!(!row.holds, "the row's verdict is its parts' verdict");
    assert_eq!(row.parts.len(), 13);
    assert!(record.render().contains("R21 › report"));

    let trace = crate::topology::census::tests::deferred_verification_trace();
    let deferred = crate::topology::census::tests::deferred_verification_fold();
    let observed = observe(
        &deferred,
        &trace,
        &PhysicalInventory::default(),
        &ProcessLocal::default(),
    );
    assert_eq!(
        observed
            .observation(Row::R14)
            .and_then(|observation| observation.part("verification_defers")),
        Some(Fact::Present(1)),
        "R14's verification_defers part is the candidate's deferral count, read from the \
         durable prefix"
    );
}

// A-12 (the PR11 decision appendix's §5.2, O4(a)): a late instance — one an
// earlier incarnation's still-running Git writer creates after terminal
// finalization's final sweep has scanned — is accounted `ResumablyOpen` in its
// slot's row. The end-to-end tests below run at the level of C's own
// final-sweep tests (T-FIN1, `src/workspace_manager/tests.rs`): a real
// repository and execution root, instances and registrations made by managers
// of other incarnations, the frozen finalizer's cleanup replayed step by step
// through the manager's public calls, and an observer at the `Before` phase of
// the final sweep's first removal. The rows A-12 changes (R9, R10, R18, R24,
// R27) are measured there; the rows it does not change read a balanced run end
// (`balanced_base`), which the frozen ledger tests measure on real runs.

/// The run's creator, whose `run_started` records it.
const CREATOR: &str = "a12-creator";
/// The coordinator that resumed the run and finalizes it.
const FINALIZER: &str = "a12-finalizer";
/// An incarnation that wrote an intent and started its add, and died before
/// any `run_resumed` recorded it (T-ACC-6's R).
const LOST: &str = "a12-lost";
/// A later resume's incarnation, which finalizes a finished run again
/// (T-ACC-5).
const RESUMER: &str = "a12-resumer";

/// A run's durable log and the fold replayed from it.
struct Run {
    fold: TopologyFold,
    events: Vec<TopologyEvent>,
}

impl Run {
    /// The census's `run_started` under the fixture's run id and `creator`, a
    /// `run_resumed` for each of `resumed`, and, where one more event makes
    /// `outcome` the fold's own derived outcome, that event: an open question
    /// for Parked, a current budget stop for BudgetExceeded. For Complete and
    /// Halted the log is a run with no work in flight, whose logical rows are
    /// those outcomes' zero and void rows.
    fn of(outcome: Outcome, creator: &str, resumed: &[&str]) -> Self {
        let started = started(creator);
        let mut events = vec![event(TopologyEventBody::RunStarted {
            data: Box::new(started.clone()),
        })];
        for incarnation in resumed {
            events.push(event(TopologyEventBody::RunResumed {
                data: Box::new(RunResumed4 {
                    incarnation: IncarnationId((*incarnation).to_owned()),
                    runner: started.runner.clone(),
                    probed_agents: started.probed_agents.clone(),
                    upstroke_version: started.upstroke_version.clone(),
                }),
            }));
        }
        match outcome {
            Outcome::Parked => events.push(event(TopologyEventBody::QuestionRaised {
                data: QuestionRaised4 {
                    question: FrozenQuestion {
                        id: QuestionId::from("a12-parked"),
                        key: TaskKey(0),
                        kind: QuestionKind::Clarify,
                        context: "the late-instance ledger's parked run".to_owned(),
                        options: vec!["continue".to_owned()],
                    },
                },
            })),
            Outcome::BudgetExceeded => {
                let epoch = replay(&events)
                    .epoch()
                    .expect("a started run is in an epoch");
                events.push(event(TopologyEventBody::BudgetExceeded {
                    data: BudgetExceeded4 {
                        epoch,
                        budget: BudgetKind::Run,
                        limit_usd: 1.0,
                        spent_usd: 1.0,
                        key: None,
                    },
                }));
            }
            Outcome::Complete | Outcome::Halted | Outcome::NoRunFinished => {}
        }
        Self {
            fold: replay(&events),
            events,
        }
    }
}

fn event(body: TopologyEventBody) -> TopologyEvent {
    TopologyEvent {
        ts: "2026-10-12T00:00:00Z".to_owned(),
        body,
    }
}

fn replay(events: &[TopologyEvent]) -> TopologyFold {
    TopologyFold::replay(crate::topology::census::tests::inputs(), events)
        .expect("the run's log replays")
}

/// The census's `run_started`, re-keyed to the fixture's run id and
/// `incarnation`, its registry digest taken again over the record it now is.
fn started(incarnation: &str) -> RunStarted4 {
    let Some(TopologyEventBody::RunStarted { data }) =
        crate::topology::census::tests::deferred_verification_trace()
            .into_iter()
            .next()
            .map(|event| event.body)
    else {
        panic!("the census trace opens with its run_started");
    };
    let mut started = *data;
    started.run_id = RUN_ID.to_owned();
    started.incarnation = IncarnationId(incarnation.to_owned());
    started.registry_digest = TaskRegistry::originals_with_agents(
        &crate::topology::census::tests::inputs().plan,
        &started.registry_record(),
        &started.probed_agents,
    )
    .expect("the census plan derives a registry")
    .digest();
    started
}

/// A manager of the fixture's run under `incarnation`.
fn manager(fixture: &Fixture, incarnation: &str) -> WorkspaceManager {
    WorkspaceManager::derive(&fixture.base, &fixture.private, RUN_ID, incarnation)
        .expect("a manager of the fixture's run under that incarnation")
}

/// The names a directory holds, sorted; none when it does not exist.
fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .expect("a directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("list {}: {error}", directory.display()),
    };
    names.sort();
    names
}

/// Every entry under the three slot namespaces: its row, its path and its
/// name, whoever created it and whether or not anything registers it.
fn instance_dirs(manager: &WorkspaceManager) -> Vec<(Row, PathBuf, String)> {
    LATE_ROWS
        .iter()
        .flat_map(|&(row, namespace)| {
            let directory = manager.execution_root().join(namespace);
            names(&directory)
                .into_iter()
                .map(move |name| (row, directory.join(&name), name))
        })
        .collect()
}

/// The at-sweep inventory's listing: every entry under the slot namespaces,
/// by row and name.
fn listing(manager: &WorkspaceManager) -> Vec<(Row, String)> {
    instance_dirs(manager)
        .into_iter()
        .map(|(row, _, name)| (row, name))
        .collect()
}

/// The objects the instance at `path` references: every object its `HEAD`
/// reaches, and every blob its index names.
fn references(path: &Path) -> Vec<String> {
    if !path.join(".git").exists() {
        return Vec::new();
    }
    let reached = git(path, &["rev-list", "--objects", "HEAD"]);
    let indexed = git(path, &["ls-files", "--stage"]);
    let mut objects: Vec<String> = reached
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .chain(
            indexed
                .lines()
                .filter_map(|line| line.split_whitespace().nth(1)),
        )
        .map(str::to_owned)
        .collect();
    objects.sort();
    objects.dedup();
    objects
}

/// Every intent file in the execution root's intents directory.
fn intent_names(manager: &WorkspaceManager) -> Vec<String> {
    names(&manager.execution_root().join("intents"))
        .into_iter()
        .filter(|name| name.ends_with(".intent"))
        .collect()
}

/// The records those intent files hold.
fn records(manager: &WorkspaceManager) -> Vec<IntentRecord> {
    let directory = manager.execution_root().join("intents");
    intent_names(manager)
        .into_iter()
        .map(|name| {
            let bytes = std::fs::read(directory.join(name)).expect("read an intent");
            serde_json::from_slice(&bytes).expect("an intent the manager wrote parses")
        })
        .collect()
}

/// A slot namespace's intents and instance directories, each instance once:
/// an intent and the directory of the instance it names are one.
fn slot_count(manager: &WorkspaceManager, namespace: &str) -> u32 {
    let prefix = format!("{namespace}.");
    let mut instances: BTreeSet<String> = intent_names(manager)
        .iter()
        .filter_map(|name| {
            name.strip_prefix(prefix.as_str())?
                .strip_suffix(".intent")
                .map(str::to_owned)
        })
        .collect();
    instances.extend(names(&manager.execution_root().join(namespace)));
    u32::try_from(instances.len()).expect("a small count")
}

fn store(repo: &Path) -> Vec<String> {
    let mut objects: Vec<String> = git(
        repo,
        &[
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objectname)",
        ],
    )
    .lines()
    .map(str::to_owned)
    .collect();
    objects.sort();
    objects
}

fn unreachable(repo: &Path) -> Vec<String> {
    let mut objects =
        crate::workspace_manager::unreachable_objects(repo).expect("fsck over the fixture");
    objects.sort();
    objects
}

/// The facts of a balanced run end in the rows A-12 does not change, for
/// which this fixture has no run directory, container or credential volume:
/// the frozen ledger tests measure those rows on real runs.
fn balanced_base() -> PhysicalInventory {
    PhysicalInventory {
        integration_ref_present: true,
        event_log_present: true,
        plan_present: true,
        report_present: true,
        owner_record_present: true,
        commit_record_present: true,
        run_lock_file_present: true,
        cleanup_lock_file_present: true,
        worktree_lock_file_present: true,
        volumes_unchanged: true,
        ..PhysicalInventory::default()
    }
}

fn process() -> ProcessLocal {
    ProcessLocal {
        invocations_balanced: true,
        ..ProcessLocal::default()
    }
}

/// One inventory the ledger takes of the run: the physical facts, measured
/// for the rows A-12 changes, and the late half's.
struct Inventory {
    physical: PhysicalInventory,
    instances: Vec<InstanceSeen>,
    intents: Vec<String>,
    records: Vec<IntentRecord>,
}

impl Inventory {
    fn take(
        fixture: &Fixture,
        manager: &WorkspaceManager,
        released: &[String],
        store_before: &[String],
    ) -> Self {
        let small = |n: usize| u32::try_from(n).expect("a small count");
        let namespace = run_namespace(RUN_ID);
        let refs = |prefix: &str| {
            small(
                manager
                    .refs_under(&format!("{namespace}{prefix}"))
                    .expect("refs")
                    .len(),
            )
        };
        let store_now = store(&fixture.base);
        Self {
            physical: PhysicalInventory {
                task_slots: slot_count(manager, "tasks"),
                staging_slots: slot_count(manager, "merge"),
                snapshot_slots: slot_count(manager, "snapshots"),
                candidates_refs: refs("candidates/"),
                prepared_pins: refs("prepared/"),
                candidate_pins: refs("candidate-prepared/"),
                execution_root_present: manager.execution_root().exists(),
                unreachable_objects: small(unreachable(&fixture.base).len()),
                store_objects: small(store_now.len()),
                store_objects_missing: small(
                    store_before
                        .iter()
                        .filter(|object| store_now.binary_search(object).is_err())
                        .count(),
                ),
                released_objects_checked: small(released.len()),
                released_objects_missing: small(
                    released
                        .iter()
                        .filter(|object| !manager.object_exists(object).unwrap_or(false))
                        .count(),
                ),
                ..balanced_base()
            },
            instances: instance_dirs(manager)
                .into_iter()
                .map(|(row, path, name)| InstanceSeen {
                    row,
                    references: references(&path),
                    name,
                })
                .collect(),
            intents: intent_names(manager),
            records: records(manager),
        }
    }
}

/// What the observer took at the final sweep's first removal: the at-sweep
/// inventory's listing and the intent records it read. Neither, when the
/// sweep removed nothing.
#[derive(Default)]
struct Taken {
    listed: Option<Vec<(Row, String)>>,
    records: Vec<IntentRecord>,
}

/// One command's ledgers, and the intent records every inventory it took
/// read, for a later command's attribution.
struct Command {
    before: Ledger,
    after: Ledger,
    records: Vec<IntentRecord>,
}

/// A command's start: its before-inventory, which A-12 takes at the start of
/// every command, with the objects then referenced, the store and the
/// unreachable set its after-inventory is compared against.
struct Start {
    released: Vec<String>,
    store: Vec<String>,
    unreachable: Vec<String>,
    before: Inventory,
}

impl Start {
    fn take(fixture: &Fixture, manager: &WorkspaceManager) -> Self {
        let mut released: Vec<String> = manager
            .refs_under(&run_namespace(RUN_ID))
            .expect("refs")
            .into_iter()
            .map(|(_, oid)| oid)
            .collect();
        released.extend(
            instance_dirs(manager)
                .into_iter()
                .filter(|(_, path, _)| path.join(".git").exists())
                .map(|(_, path, _)| git(&path, &["rev-parse", "HEAD"])),
        );
        released.sort();
        released.dedup();
        let store = store(&fixture.base);
        let unreachable = unreachable(&fixture.base);
        let before = Inventory::take(fixture, manager, &released, &store);
        Self {
            released,
            store,
            unreachable,
            before,
        }
    }

    /// The command's before and after ledgers, its after-inventory taken now.
    /// Attribution reads the intent records of every inventory the ledger took
    /// of the run: `earlier` commands', and this command's before, at-sweep
    /// and after inventories'.
    fn ledgers(
        &self,
        fixture: &Fixture,
        manager: &WorkspaceManager,
        run: &Run,
        earlier: &[IntentRecord],
        taken: Taken,
    ) -> Command {
        let inventory = Inventory::take(fixture, manager, &self.released, &self.store);
        let mut records = earlier.to_vec();
        records.extend(self.before.records.iter().cloned());
        records.extend(taken.records);
        records.extend(inventory.records.iter().cloned());
        let before = observe_late(
            &run.fold,
            &run.events,
            &self.before.physical,
            &LateInventory {
                instances: self.before.instances.clone(),
                intents: self.before.intents.clone(),
                at_sweep: None,
                records: records.clone(),
                unreachable_before: Vec::new(),
            },
            &process(),
        );
        let after = observe_late(
            &run.fold,
            &run.events,
            &inventory.physical,
            &LateInventory {
                instances: inventory.instances,
                intents: inventory.intents,
                at_sweep: taken.listed,
                records: records.clone(),
                unreachable_before: self.unreachable.clone(),
            },
            &process(),
        );
        Command {
            before,
            after,
            records,
        }
    }
}

/// What an earlier incarnation's Git writer that had not yet run does once it
/// runs: `git worktree add --detach` at `path`, which makes the leading
/// directories, the checkout and the registration, and writes no intent.
fn late_add(fixture: &Fixture, path: &Path, commit: &str) {
    git_os(
        &fixture.base,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("-q"),
            OsStr::new("--detach"),
            path.as_os_str(),
            OsStr::new(commit),
        ],
    );
}

/// An earlier incarnation's instance planted as T-FIN1 plants one: its
/// checkout and registration at `head`, and no intent.
fn plant(fixture: &Fixture, path: &Path) {
    late_add(fixture, path, &fixture.head);
}

/// The test's hook observer at terminal finalization's last step: at the
/// `Before` phase of the final sweep's first removal it takes the at-sweep
/// inventory, through the manager's effect-hook seam, and then makes its late
/// adds, after the sweep's scan and before the root's emptiness check (C
/// §5.4's second row). Every phase proceeds.
struct AtSweep<'a> {
    fixture: &'a Fixture,
    manager: &'a WorkspaceManager,
    adds: Vec<(PathBuf, String)>,
    taken: Option<Taken>,
}

impl<'a> AtSweep<'a> {
    fn new(
        fixture: &'a Fixture,
        manager: &'a WorkspaceManager,
        adds: Vec<(PathBuf, String)>,
    ) -> Self {
        Self {
            fixture,
            manager,
            adds,
            taken: None,
        }
    }

    fn taken(self) -> Taken {
        self.taken.unwrap_or_default()
    }
}

impl EffectHooks for AtSweep<'_> {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        let removal = matches!(
            site,
            EffectSiteId::Worktree(WorktreeSite::Remove | WorktreeSite::RemoveStaging)
                | EffectSiteId::Snapshot(SnapshotSite::Remove)
        );
        if removal && phase == HookPhase::Before && self.taken.is_none() {
            self.taken = Some(Taken {
                listed: Some(listing(self.manager)),
                records: records(self.manager),
            });
            for (path, commit) in &self.adds {
                late_add(self.fixture, path, commit);
            }
        }
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }
}

/// One frozen scrub step, `finalize.rs`'s private `scrub_slots`, through the
/// manager's public calls: for every slot the enumeration returns that the
/// step keeps, the forced removal under the finalizer's proof, then the
/// intent.
fn scrub(manager: &WorkspaceManager, keep: fn(&Slot) -> bool) {
    for slot in manager
        .intents_pausing(&mut NoHooks)
        .expect("the scrub's enumeration")
    {
        if !keep(&slot) {
            continue;
        }
        let passed_over = manager
            .remove_worktree_proving(&mut NoHooks, &slot, WriterProof::NoWriterAlive)
            .expect("the scrub's removal");
        assert!(
            passed_over.is_empty(),
            "no registration here names nothing: {passed_over:?}"
        );
        manager
            .remove_intent(&mut NoHooks, &slot)
            .expect("the scrub's intent removal");
    }
}

/// `finalize.rs`'s private `delete_refs_under`, through the manager's public
/// call.
fn delete_refs(manager: &WorkspaceManager, prefix: &str, site: RefSite) {
    for (refname, oid) in manager.refs_under(prefix).expect("refs") {
        manager
            .delete_ref_expected_old(&mut NoHooks, site, &refname, &oid)
            .expect("the ref's deletion");
    }
}

/// Terminal finalization's cleanup for `outcome`, in the frozen finalizer's
/// order (`CleanupStep::ORDER`, `CleanupStep::applies_to`) and through the
/// calls its private steps make. `plant` runs after the last scrub, before
/// the step that holds the final sweep, so that only the sweep can discover
/// what it plants; `hooks` is that step's. Returns what the finalizer reports
/// as `execution_root_removed`.
fn finalize_as(
    manager: &WorkspaceManager,
    outcome: &RunOutcome,
    plant: impl FnOnce(),
    hooks: &mut dyn EffectHooks,
) -> bool {
    let namespace = run_namespace(RUN_ID);
    let mut plant = Some(plant);
    let mut execution_root_removed = false;
    for step in CleanupStep::ORDER {
        if !step.applies_to(outcome) {
            continue;
        }
        match step {
            CleanupStep::TaskWorktrees => scrub(manager, |slot| matches!(slot, Slot::Task { .. })),
            CleanupStep::Snapshots => scrub(manager, |slot| matches!(slot, Slot::Snapshot { .. })),
            CleanupStep::Staging => scrub(manager, |slot| matches!(slot, Slot::Staging { .. })),
            CleanupStep::Pins => {
                delete_refs(
                    manager,
                    &format!("{namespace}prepared/"),
                    RefSite::DeletePreparedPin,
                );
                delete_refs(
                    manager,
                    &format!("{namespace}candidate-prepared/"),
                    RefSite::DeleteCandidatePin,
                );
            }
            CleanupStep::CandidatesRefs => delete_refs(
                manager,
                &format!("{namespace}candidates/"),
                RefSite::DeleteCandidatesRef,
            ),
            CleanupStep::ExecutionRoot => {
                if let Some(plant) = plant.take() {
                    plant();
                }
                manager
                    .remove_staging_leftovers(&mut NoHooks)
                    .expect("the staging leftovers");
                execution_root_removed = manager
                    .remove_execution_root(hooks)
                    .expect("finalization's last step");
            }
        }
    }
    execution_root_removed
}

/// An earlier incarnation's ephemeral snapshot commit that nothing references:
/// a new blob, a tree holding it and a commit of that tree on `head`, written
/// through the base checkout's index, which then lets go of them. Returns the
/// commit and its three objects, sorted.
fn ephemeral_commit(fixture: &Fixture) -> (String, Vec<String>) {
    let file = fixture.base.join("a12-ephemeral.txt");
    write_file(&file, b"an earlier incarnation's ephemeral snapshot\n");
    git(&fixture.base, &["add", "a12-ephemeral.txt"]);
    let blob = git(&fixture.base, &["rev-parse", ":a12-ephemeral.txt"]);
    let tree = git(&fixture.base, &["write-tree"]);
    let commit = git(
        &fixture.base,
        &[
            "commit-tree",
            tree.as_str(),
            "-p",
            fixture.head.as_str(),
            "-m",
            "an ephemeral snapshot commit",
        ],
    );
    git(
        &fixture.base,
        &["rm", "-q", "--cached", "a12-ephemeral.txt"],
    );
    remove_file(&file);
    let mut objects = vec![blob, tree, commit.clone()];
    objects.sort();
    let unreachable = unreachable(&fixture.base);
    assert!(
        objects
            .iter()
            .all(|object| unreachable.binary_search(object).is_ok()),
        "nothing references the commit, its tree or its blob: {objects:?} against {unreachable:?}"
    );
    (commit, objects)
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .expect("a slot instance's name is UTF-8")
        .to_owned()
}

#[track_caller]
fn part_of(ledger: &Ledger, row: Row, name: &str) -> Fact {
    ledger
        .observation(row)
        .and_then(|observation| observation.part(name))
        .unwrap_or_else(|| panic!("{row} carries its `{name}` part"))
}

fn disagreeing(disagreements: &[Disagreement]) -> Vec<(Row, Option<&'static str>)> {
    disagreements
        .iter()
        .map(|disagreement| (disagreement.row, disagreement.part))
        .collect()
}

#[track_caller]
fn assert_agrees(command: &Command, outcome: Outcome, what: &str) {
    let disagreements = check(&command.before, &command.after, outcome);
    assert!(
        disagreements.is_empty(),
        "{what}: the {outcome:?} equation disagrees:\n{disagreements:#?}\n\n{}",
        record(&command.before, &command.after, outcome).render()
    );
}

/// The row `record` exports for `row` from a before and an after ledger.
#[track_caller]
fn exported(before: &Ledger, after: &Ledger, outcome: Outcome, row: Row) -> LedgerRecordRow {
    record(before, after, outcome)
        .rows
        .into_iter()
        .find(|exported| exported.row == row)
        .unwrap_or_else(|| panic!("{row} is exported"))
}

/// T-ACC-1 (A-12): an earlier incarnation's instance is planted after the
/// finalizer's task scrub, as T-FIN1 plants one, so that the final sweep has a
/// removal to make. At the `Before` phase of that removal the observer takes
/// the at-sweep inventory, and then a second manager's writer, the creator's,
/// adds a task instance: after the sweep's scan and before its emptiness check
/// (C §5.4's second row). Finalization reports `execution_root_removed:
/// false`; R9's late part is one instance, attributed to the creator through
/// `run_started`; R18 is present; and `check` agrees. M-ACC-1, the census
/// attributing nothing, turns it red.
#[test]
fn a_late_instance_added_after_the_sweeps_scan_is_accounted_resumably_open_in_its_slots_row() {
    let fixture = Fixture::created("a12-t-acc-1");
    let run = Run::of(Outcome::Complete, CREATOR, &[FINALIZER]);
    let creator = manager(&fixture, CREATOR);
    let finalizer = manager(&fixture, FINALIZER);
    let start = Start::take(&fixture, &finalizer);
    let planted = creator.slot_path(&fixture.task("alpha", 1));
    let late = creator.slot_path(&fixture.task("beta", 1));
    let mut sweep = AtSweep::new(
        &fixture,
        &finalizer,
        vec![(late.clone(), fixture.head.clone())],
    );
    let removed = finalize_as(
        &finalizer,
        &RunOutcome::Complete,
        || plant(&fixture, &planted),
        &mut sweep,
    );
    let taken = sweep.taken();
    assert!(
        !removed,
        "finalization returns execution_root_removed: false"
    );
    assert!(
        !planted.exists() && late.join(".git").exists(),
        "the sweep removed the planted instance, and the late one stands"
    );
    assert_eq!(
        taken.listed,
        Some(vec![(Row::R9, name_of(&planted))]),
        "the at-sweep inventory lists the instance the sweep removes, and not the late one"
    );
    let command = start.ledgers(&fixture, &finalizer, &run, &[], taken);
    assert_eq!(
        part_of(&command.after, Row::R9, LATE_PART),
        Fact::Late {
            crossed: 1,
            attributed: 1
        },
        "R9's late part is the one instance, attributed"
    );
    assert_eq!(command.after.fact(Row::R9), Some(Fact::Absent));
    assert_eq!(command.after.fact(Row::R18), Some(Fact::Present(1)));
    assert!(command.after.at_sweep_inventory);
    assert_agrees(&command, Outcome::Complete, "T-ACC-1");
}

/// T-ACC-2 (A-12, the check alone): R18 present with no late instance — the
/// at-sweep inventory taken and nothing late — is a disagreement, and the
/// only one. M-ACC-2, R18 admitting any presence, makes it agree: red.
#[test]
fn an_execution_root_present_with_no_late_instance_is_a_disagreement() {
    let run = Run::of(Outcome::Complete, CREATOR, &[FINALIZER]);
    let kept = PhysicalInventory {
        execution_root_present: true,
        ..balanced_base()
    };
    let before = observe_late(
        &run.fold,
        &run.events,
        &kept,
        &LateInventory::default(),
        &process(),
    );
    let after = observe_late(
        &run.fold,
        &run.events,
        &kept,
        &LateInventory {
            at_sweep: Some(Vec::new()),
            ..LateInventory::default()
        },
        &process(),
    );
    let disagreements = check(&before, &after, Outcome::Complete);
    assert_eq!(
        disagreeing(&disagreements),
        vec![(Row::R18, None)],
        "{disagreements:#?}"
    );
}

/// T-ACC-3 (A-12): an earlier incarnation's ephemeral snapshot commit is
/// unreachable before the command; at the final sweep's first removal a late
/// snapshot add points its `HEAD` at it; after finalization the unreachable
/// count has fallen by the commit's three objects, `late_referenced_objects`
/// is that fall, and `check` agrees. M-ACC-3, R27's `unreachable_objects`
/// plain `Monotone`, turns it red.
#[test]
fn a_late_snapshot_add_that_makes_an_ephemeral_commit_reachable_again_is_accounted() {
    let fixture = Fixture::created("a12-t-acc-3");
    let run = Run::of(Outcome::Complete, CREATOR, &[FINALIZER]);
    let creator = manager(&fixture, CREATOR);
    let finalizer = manager(&fixture, FINALIZER);
    let (ephemeral, objects) = ephemeral_commit(&fixture);
    let start = Start::take(&fixture, &finalizer);
    let planted = creator.slot_path(&fixture.task("alpha", 1));
    let snapshot = creator.slot_path(&Slot::Snapshot {
        name: SnapshotName::integration(1),
    });
    let mut sweep = AtSweep::new(&fixture, &finalizer, vec![(snapshot.clone(), ephemeral)]);
    let removed = finalize_as(
        &finalizer,
        &RunOutcome::Complete,
        || plant(&fixture, &planted),
        &mut sweep,
    );
    let taken = sweep.taken();
    assert!(!removed && snapshot.join(".git").exists());
    let command = start.ledgers(&fixture, &finalizer, &run, &[], taken);
    let unreachable_in = |ledger: &Ledger| part_of(ledger, Row::R27, "unreachable_objects").count();
    let three = u32::try_from(objects.len()).expect("three objects");
    assert_eq!(
        unreachable_in(&command.before) - unreachable_in(&command.after),
        three,
        "the unreachable count falls by the commit's objects: {objects:?}"
    );
    assert_eq!(
        part_of(&command.after, Row::R27, LATE_OBJECTS),
        Fact::Present(three),
        "late_referenced_objects is that fall"
    );
    assert_eq!(
        part_of(&command.after, Row::R24, LATE_PART),
        Fact::Late {
            crossed: 1,
            attributed: 1
        }
    );
    assert_agrees(&command, Outcome::Complete, "T-ACC-3");
}

/// T-ACC-4 (A-12, the check alone): an unreachable decrease with no late
/// instance is a disagreement on R27's `unreachable_objects`, and the only
/// one. M-ACC-4, any decrease admitted, makes it agree: red.
#[test]
fn an_unreachable_decrease_with_no_late_instance_is_a_disagreement() {
    let run = Run::of(Outcome::Complete, CREATOR, &[FINALIZER]);
    let before = observe_late(
        &run.fold,
        &run.events,
        &PhysicalInventory {
            unreachable_objects: 3,
            ..balanced_base()
        },
        &LateInventory::default(),
        &process(),
    );
    let after = observe_late(
        &run.fold,
        &run.events,
        &PhysicalInventory {
            unreachable_objects: 1,
            ..balanced_base()
        },
        &LateInventory {
            at_sweep: Some(Vec::new()),
            ..LateInventory::default()
        },
        &process(),
    );
    let disagreements = check(&before, &after, Outcome::Complete);
    assert_eq!(
        disagreeing(&disagreements),
        vec![(Row::R27, Some("unreachable_objects"))],
        "{disagreements:#?}"
    );
}

/// T-ACC-5 (A-12): a Halted run whose finalization a late task instance
/// crossed — the planted instance a snapshot, so that the sweep, not a scrub,
/// takes the at-sweep inventory — keeps its root, the late instance accounted
/// `ResumablyOpen` and the root it keeps exported `ResumablyOpen` too (A-4,
/// A-7); its resume finalizes again, and after it `late_instances` is zero, R18
/// absent and exported `Pruned`, and `check` agrees. M-ACC-5, C's discovery
/// passing over earlier incarnations' task instances, leaves the late instance
/// to the resume: red. M-A12-R1, R18's kept class dropped, exports the kept
/// root `Pruned`: red.
#[test]
fn a_halted_runs_late_instance_is_reclaimed_by_its_resumes_finalization() {
    let fixture = Fixture::created("a12-t-acc-5");
    let run = Run::of(Outcome::Halted, CREATOR, &[FINALIZER]);
    let creator = manager(&fixture, CREATOR);
    let finalizer = manager(&fixture, FINALIZER);
    let start = Start::take(&fixture, &finalizer);
    let planted = creator.slot_path(&Slot::Snapshot {
        name: SnapshotName::integration(1),
    });
    let late = creator.slot_path(&fixture.task("beta", 1));
    let mut sweep = AtSweep::new(
        &fixture,
        &finalizer,
        vec![(late.clone(), fixture.head.clone())],
    );
    let removed = finalize_as(
        &finalizer,
        &RunOutcome::Halted,
        || plant(&fixture, &planted),
        &mut sweep,
    );
    let taken = sweep.taken();
    assert!(!removed && late.join(".git").exists());
    let halted = start.ledgers(&fixture, &finalizer, &run, &[], taken);
    assert_eq!(
        part_of(&halted.after, Row::R9, LATE_PART),
        Fact::Late {
            crossed: 1,
            attributed: 1
        }
    );
    assert_eq!(halted.after.fact(Row::R18), Some(Fact::Present(1)));
    assert_agrees(&halted, Outcome::Halted, "the Halted run");
    assert_eq!(
        exported(&halted.before, &halted.after, Outcome::Halted, Row::R18).class,
        Class::ResumablyOpen,
        "the Halted run's root, kept by its late instance, is resumably open until the resume's \
         finalization reclaims it"
    );

    let resumer = manager(&fixture, RESUMER);
    let again = Start::take(&fixture, &resumer);
    let mut sweep = AtSweep::new(&fixture, &resumer, Vec::new());
    let removed = finalize_as(&resumer, &RunOutcome::Halted, || {}, &mut sweep);
    let taken = sweep.taken();
    assert!(
        removed && !late.exists(),
        "the resume's finalization reclaims the late instance, and the root"
    );
    let resumed = again.ledgers(&fixture, &resumer, &run, &halted.records, taken);
    assert_eq!(
        part_of(&resumed.after, Row::R9, LATE_PART),
        Fact::Late {
            crossed: 0,
            attributed: 0
        }
    );
    assert_eq!(resumed.after.fact(Row::R18), Some(Fact::Absent));
    assert_agrees(&resumed, Outcome::Halted, "its resume");
    assert_eq!(
        exported(&resumed.before, &resumed.after, Outcome::Halted, Row::R18).class,
        Class::Pruned,
        "the resume's finalization reclaims the root: pruned"
    );
}

/// T-ACC-6 (A-12, the attribution rule): a manager with incarnation R writes a
/// task intent and starts its add, as recovery's step (g) does, and no
/// `run_resumed` records R. The successor S's command takes its
/// before-inventory, which reads R's intent; S's recovery reclaims R's intent
/// and instance; and at the `Before` phase of S's final sweep's first removal
/// (of an instance planted as in T-ACC-1), after the at-sweep inventory, R's
/// delayed Git child adds R's instance. After finalization it is a late
/// instance attributed to R through S's before-inventory, and `check` agrees.
/// M-ACC-6, attribution from `run_started` and `run_resumed` only, leaves it
/// unattributed: `check` disagrees, red.
#[test]
fn a_late_instance_of_an_incarnation_no_run_resumed_records_is_attributed_through_its_intent() {
    let fixture = Fixture::created("a12-t-acc-6");
    let run = Run::of(Outcome::Complete, CREATOR, &[FINALIZER]);
    let creator = manager(&fixture, CREATOR);
    let lost = manager(&fixture, LOST);
    let successor = manager(&fixture, FINALIZER);
    let alpha = fixture.task("alpha", 1);
    lost.write_intent(&mut NoHooks, &alpha)
        .expect("R's intent, synced before its Git child starts");
    let start = Start::take(&fixture, &successor);
    assert!(
        start
            .before
            .records
            .iter()
            .any(|record| record.incarnation() == LOST),
        "S's before-inventory reads R's intent"
    );
    successor
        .remove_worktree(&mut NoHooks, &alpha)
        .expect("S's recovery reclaims R's instance");
    successor
        .remove_intent(&mut NoHooks, &alpha)
        .expect("and R's intent");
    let planted = creator.slot_path(&fixture.task("beta", 1));
    let late = lost.slot_path(&alpha);
    let mut sweep = AtSweep::new(
        &fixture,
        &successor,
        vec![(late.clone(), fixture.head.clone())],
    );
    let removed = finalize_as(
        &successor,
        &RunOutcome::Complete,
        || plant(&fixture, &planted),
        &mut sweep,
    );
    let taken = sweep.taken();
    assert!(!removed && late.join(".git").exists());
    assert!(
        taken
            .records
            .iter()
            .all(|record| record.incarnation() != LOST),
        "R's intent was gone by the sweep: only S's before-inventory read it"
    );
    let command = start.ledgers(&fixture, &successor, &run, &[], taken);
    assert_agrees(&command, Outcome::Complete, "T-ACC-6");
    assert_eq!(
        part_of(&command.after, Row::R9, LATE_PART),
        Fact::Late {
            crossed: 1,
            attributed: 1
        },
        "R's instance is late, attributed to R"
    );
}

/// T-ACC-7 (A-12, the at-sweep rule end to end): an earlier incarnation's task
/// and staging instances, with no intent, are planted after the finalizer's
/// scrubs, so that the sweep can discover both; the at-sweep inventory lists
/// both; finalization removes both and the root; `late_instances` is zero and
/// `check` agrees. M-SWEEP (M-ACC-7), C's sweep passing over staging
/// instances, leaves the staging instance, which the at-sweep inventory lists:
/// `check` disagrees, red.
#[test]
fn earlier_instances_the_sweep_can_discover_are_removed_and_none_is_late() {
    let fixture = Fixture::created("a12-t-acc-7");
    let run = Run::of(Outcome::Complete, CREATOR, &[FINALIZER]);
    let creator = manager(&fixture, CREATOR);
    let finalizer = manager(&fixture, FINALIZER);
    let start = Start::take(&fixture, &finalizer);
    let task = creator.slot_path(&fixture.task("alpha", 1));
    let staging = creator.slot_path(&Slot::Staging { sequence: 1 });
    let mut sweep = AtSweep::new(&fixture, &finalizer, Vec::new());
    let removed = finalize_as(
        &finalizer,
        &RunOutcome::Complete,
        || {
            plant(&fixture, &task);
            plant(&fixture, &staging);
        },
        &mut sweep,
    );
    let taken = sweep.taken();
    assert_eq!(
        taken.listed,
        Some(vec![
            (Row::R9, name_of(&task)),
            (Row::R10, name_of(&staging)),
        ]),
        "the at-sweep inventory lists both instances"
    );
    let command = start.ledgers(&fixture, &finalizer, &run, &[], taken);
    assert_agrees(&command, Outcome::Complete, "T-ACC-7");
    assert!(removed && !task.exists() && !staging.exists());
    for row in [Row::R9, Row::R10] {
        assert_eq!(
            part_of(&command.after, row, LATE_PART),
            Fact::Late {
                crossed: 0,
                attributed: 0
            },
            "{row}"
        );
    }
}

/// T-ACC-8 (A-12, the at-sweep rule, the check alone): observations in which
/// an earlier incarnation's instance with no intent is listed in the at-sweep
/// inventory and present after finalization, keeping the root. It is not
/// late: it is residue the sweep could discover and left, and `check`
/// disagrees on R9 and on R18. M-ACC-8, the late predicate without the
/// at-sweep condition, makes it agree: red.
#[test]
fn an_intentless_instance_the_at_sweep_inventory_lists_and_finalization_left_is_a_disagreement() {
    let run = Run::of(Outcome::Complete, CREATOR, &[FINALIZER]);
    let name = format!(
        "kalpha-g1_{}",
        InstanceTag::of_incarnation(CREATOR).as_str()
    );
    let before = observe_late(
        &run.fold,
        &run.events,
        &balanced_base(),
        &LateInventory::default(),
        &process(),
    );
    let after = observe_late(
        &run.fold,
        &run.events,
        &PhysicalInventory {
            task_slots: 1,
            execution_root_present: true,
            ..balanced_base()
        },
        &LateInventory {
            instances: vec![InstanceSeen {
                row: Row::R9,
                name: name.clone(),
                references: Vec::new(),
            }],
            intents: Vec::new(),
            at_sweep: Some(vec![(Row::R9, name)]),
            records: Vec::new(),
            unreachable_before: Vec::new(),
        },
        &process(),
    );
    let disagreements = check(&before, &after, Outcome::Complete);
    assert_eq!(
        disagreeing(&disagreements),
        vec![(Row::R9, None), (Row::R18, None)],
        "{disagreements:#?}"
    );
    assert_eq!(
        part_of(&after, Row::R9, LATE_PART),
        Fact::Late {
            crossed: 0,
            attributed: 0
        }
    );
}

/// A12-R1 (A-4, A-6, the check and the export alone): R18 is one fact whose
/// class follows the branch that holds, in each of the four ended outcomes. A
/// root a late instance keeps is exported `ResumablyOpen` and holds; a removed
/// root is exported `Pruned` and holds; a root present beside no late instance
/// is exported `Pruned`, does not hold, and is `check`'s one disagreement,
/// classed `Pruned`. Each outcome's equation states R18 `Pruned`,
/// `AbsentUnlessLate`, kept `ResumablyOpen`; NoRunFinished's R18 is unchanged.
/// M-A12-R1, R18's kept class dropped, restores the unconditional `Pruned`:
/// red.
#[test]
fn the_root_a_late_instance_keeps_is_resumably_open_and_otherwise_pruned_in_every_ended_outcome() {
    let late_task = format!("kbeta-g1_{}", InstanceTag::of_incarnation(CREATOR).as_str());
    for outcome in [
        Outcome::Complete,
        Outcome::Parked,
        Outcome::Halted,
        Outcome::BudgetExceeded,
    ] {
        let run = Run::of(outcome, CREATOR, &[FINALIZER]);
        let ledger = |physical: PhysicalInventory, late: LateInventory| {
            observe_late(&run.fold, &run.events, &physical, &late, &process())
        };
        let swept = || LateInventory {
            at_sweep: Some(Vec::new()),
            ..LateInventory::default()
        };
        let present = || PhysicalInventory {
            execution_root_present: true,
            ..balanced_base()
        };
        let before = ledger(present(), LateInventory::default());
        let kept = ledger(
            PhysicalInventory {
                task_slots: 1,
                ..present()
            },
            LateInventory {
                instances: vec![InstanceSeen {
                    row: Row::R9,
                    name: late_task.clone(),
                    references: Vec::new(),
                }],
                ..swept()
            },
        );
        let removed = ledger(balanced_base(), swept());
        let left = ledger(present(), swept());
        for (after, class, holds, disagreements, what) in [
            (
                &kept,
                Class::ResumablyOpen,
                true,
                0,
                "kept by a late instance",
            ),
            (&removed, Class::Pruned, true, 0, "removed"),
            (
                &left,
                Class::Pruned,
                false,
                1,
                "present beside no late instance",
            ),
        ] {
            let root = exported(&before, after, outcome, Row::R18);
            assert_eq!(
                (root.class, root.holds),
                (class, holds),
                "{outcome:?}, the root {what}: {root:?}"
            );
            assert!(
                record(&before, after, outcome).render().contains(&format!(
                    "| R18 | execution root directory | {class:?} | AbsentUnlessLate |"
                )),
                "{outcome:?}, the root {what}: the table prints its class"
            );
            let found = check(&before, after, outcome);
            assert_eq!(
                found
                    .iter()
                    .map(|disagreement| (disagreement.row, disagreement.part, disagreement.class))
                    .collect::<Vec<_>>(),
                vec![(Row::R18, None, Class::Pruned); disagreements],
                "{outcome:?}, the root {what}: {found:#?}"
            );
        }
        let root = equation(outcome)
            .into_iter()
            .find(|expectation| expectation.row == Row::R18)
            .expect("R18 is in every equation");
        assert_eq!(
            (root.class, root.holds, root.kept),
            (
                Class::Pruned,
                Requirement::AbsentUnlessLate,
                Some(Class::ResumablyOpen)
            ),
            "{outcome:?}: pruned unless a late instance keeps or recreates it (then resumably_open)"
        );
    }
    let root = equation(Outcome::NoRunFinished)
        .into_iter()
        .find(|expectation| expectation.row == Row::R18)
        .expect("R18 is in every equation");
    assert_eq!(
        (root.class, root.holds, root.kept),
        (Class::ResumablyOpen, Requirement::Present, None),
        "NoRunFinished's R18 is unchanged"
    );
}

/// The ST-18/G6 evidence (A-12's last bullet): every ended outcome with a late
/// task, staging and snapshot instance crossing the final sweep, its at-sweep
/// inventory taken; one late instance of an incarnation that never reached
/// `run_resumed` (T-ACC-6's shape); and the R27 transition, the snapshot's
/// `HEAD` making an ephemeral commit reachable again. The root the late
/// instances keep is exported `ResumablyOpen` in each outcome (A-4, A-6;
/// M-A12-R1 turns it red). Each outcome's table is written to
/// `$UPSTROKE_LEDGER_EXPORT/ledger-late-<outcome>.md` when the variable names a
/// directory, for the G6 ledger artifact.
#[test]
fn every_ended_outcome_balances_with_late_instances_crossing_the_final_sweep() {
    for (outcome, finished, tag) in [
        (Outcome::Complete, RunOutcome::Complete, "complete"),
        (Outcome::Parked, RunOutcome::Parked, "parked"),
        (Outcome::Halted, RunOutcome::Halted, "halted"),
        (
            Outcome::BudgetExceeded,
            RunOutcome::BudgetExceeded,
            "budget-exceeded",
        ),
    ] {
        let fixture = Fixture::created(&format!("a12-st18-{tag}"));
        let run = Run::of(outcome, CREATOR, &[FINALIZER]);
        let creator = manager(&fixture, CREATOR);
        let lost = manager(&fixture, LOST);
        let successor = manager(&fixture, FINALIZER);
        let alpha = fixture.task("alpha", 1);
        lost.write_intent(&mut NoHooks, &alpha)
            .expect("R's intent, synced before its Git child starts");
        let (ephemeral, objects) = ephemeral_commit(&fixture);
        let start = Start::take(&fixture, &successor);
        successor
            .remove_worktree(&mut NoHooks, &alpha)
            .expect("S's recovery reclaims R's instance");
        successor
            .remove_intent(&mut NoHooks, &alpha)
            .expect("and R's intent");
        let planted = creator.slot_path(&fixture.task("gamma", 1));
        let adds = vec![
            (
                creator.slot_path(&fixture.task("beta", 1)),
                fixture.head.clone(),
            ),
            (
                creator.slot_path(&Slot::Staging { sequence: 1 }),
                fixture.head.clone(),
            ),
            (
                creator.slot_path(&Slot::Snapshot {
                    name: SnapshotName::integration(1),
                }),
                ephemeral,
            ),
            (lost.slot_path(&alpha), fixture.head.clone()),
        ];
        let mut sweep = AtSweep::new(&fixture, &successor, adds);
        let removed = finalize_as(
            &successor,
            &finished,
            || plant(&fixture, &planted),
            &mut sweep,
        );
        let taken = sweep.taken();
        let command = start.ledgers(&fixture, &successor, &run, &[], taken);
        if let Ok(directory) = std::env::var("UPSTROKE_LEDGER_EXPORT") {
            write_file(
                &PathBuf::from(directory).join(format!("ledger-late-{tag}.md")),
                record(&command.before, &command.after, outcome)
                    .render()
                    .as_bytes(),
            );
        }
        assert_agrees(&command, outcome, tag);
        assert!(!removed, "{tag}: the late instances keep the root");
        assert!(command.after.at_sweep_inventory, "{tag}");
        for (row, crossed) in [(Row::R9, 2), (Row::R10, 1), (Row::R24, 1)] {
            assert_eq!(
                part_of(&command.after, row, LATE_PART),
                Fact::Late {
                    crossed,
                    attributed: crossed
                },
                "{tag}: {row}"
            );
        }
        assert_eq!(
            command.after.fact(Row::R18),
            Some(Fact::Present(1)),
            "{tag}"
        );
        let root = exported(&command.before, &command.after, outcome, Row::R18);
        assert_eq!(
            (root.class, root.after, root.holds),
            (Class::ResumablyOpen, Fact::Present(1), true),
            "{tag}: the root the late instances keep is exported resumably open (A-4, A-6): \
             {root:?}"
        );
        let three = u32::try_from(objects.len()).expect("three objects");
        assert_eq!(
            part_of(&command.after, Row::R27, LATE_OBJECTS),
            Fact::Present(three),
            "{tag}: the R27 transition"
        );
        assert_eq!(
            part_of(&command.before, Row::R27, "unreachable_objects").count()
                - part_of(&command.after, Row::R27, "unreachable_objects").count(),
            three,
            "{tag}: the unreachable count falls by the ephemeral commit's objects"
        );
    }
}
