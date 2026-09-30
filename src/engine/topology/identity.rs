//! Extended notes: `docs/internals/engine/topology/identity.md`

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::num::NonZeroU32;

use crate::error::UpstrokeError;
use crate::runner::invocation::{AttemptRole, SequenceRole};
use crate::runner::{AgentId, InvocationId, ProbeTarget};
use crate::topology::events::{AttemptNumber, GenerationId, SequenceId};
use crate::topology::registry::TaskKey;

use super::select::{Entitlements, MERGE_ENTITLEMENTS, Standing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttemptIdentities {
    key: TaskKey,
    generation: GenerationId,
    attempt: AttemptNumber,
}

impl AttemptIdentities {
    #[must_use]
    pub const fn new(key: TaskKey, generation: GenerationId, attempt: AttemptNumber) -> Self {
        Self {
            key,
            generation,
            attempt,
        }
    }

    #[must_use]
    pub const fn worker(&self) -> InvocationId {
        InvocationId::attempt(
            self.key,
            self.generation,
            self.attempt,
            AttemptRole::Worker,
            0,
        )
    }

    #[must_use]
    pub const fn gate(&self, gate: u32, ordinal: u32) -> InvocationId {
        InvocationId::attempt(
            self.key,
            self.generation,
            self.attempt,
            AttemptRole::Gate(gate),
            ordinal,
        )
    }

    #[must_use]
    pub const fn review_pass(&self, pass: u32, ordinal: u32) -> InvocationId {
        InvocationId::attempt(
            self.key,
            self.generation,
            self.attempt,
            AttemptRole::ReviewPass(pass),
            ordinal,
        )
    }

    #[must_use]
    pub const fn review_reask(&self, reask: u32, ordinal: u32) -> InvocationId {
        InvocationId::attempt(
            self.key,
            self.generation,
            self.attempt,
            AttemptRole::ReviewReask(reask),
            ordinal,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceIdentities {
    sequence: SequenceId,
}

impl SequenceIdentities {
    #[must_use]
    pub const fn new(sequence: SequenceId) -> Self {
        Self { sequence }
    }

    #[must_use]
    pub const fn gate(&self, gate: u32, ordinal: u32) -> InvocationId {
        InvocationId::sequence(self.sequence, SequenceRole::Gate(gate), ordinal)
    }

    #[must_use]
    pub const fn review_pass(&self, pass: u32, ordinal: u32) -> InvocationId {
        InvocationId::sequence(self.sequence, SequenceRole::ReviewPass(pass), ordinal)
    }

    #[must_use]
    pub const fn review_reask(&self, reask: u32, ordinal: u32) -> InvocationId {
        InvocationId::sequence(self.sequence, SequenceRole::ReviewReask(reask), ordinal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreflightIdentities;

impl PreflightIdentities {
    pub fn shell(ordinal: u32) -> Result<InvocationId, UpstrokeError> {
        InvocationId::probe(ProbeTarget::Shell, ordinal)
    }

    pub fn agent(agent: &str, ordinal: u32) -> Result<InvocationId, UpstrokeError> {
        InvocationId::probe(ProbeTarget::Agent(AgentId::new(agent)), ordinal)
    }
}

#[must_use]
pub fn is_slotted(invocation: &InvocationId) -> bool {
    match invocation {
        InvocationId::Attempt { role, .. } => !matches!(role, AttemptRole::Gate(_)),
        InvocationId::Sequence { role, .. } => !matches!(role, SequenceRole::Gate(_)),
        InvocationId::Probe { target, .. } => matches!(target, ProbeTarget::Agent(_)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPair {
    pub agent: String,
    pub pool: Option<String>,
}

impl fmt::Display for SlotPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.pool {
            Some(pool) => write!(f, "{{agent `{}`, pool `{pool}`}}", self.agent),
            None => write!(f, "{{agent `{}`}}", self.agent),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotLimits {
    per_agent: NonZeroU32,
    per_pool: NonZeroU32,
}

impl SlotLimits {
    pub const WIDTH_ONE: Self = Self {
        per_agent: NonZeroU32::MIN,
        per_pool: NonZeroU32::MIN,
    };

    pub fn new(per_agent: u32, per_pool: u32) -> Result<Self, UpstrokeError> {
        let limit = |key: &str, value: u32| {
            NonZeroU32::new(value).ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "`{key} = 0` is not a slot limit: a table that grants no pair holds every \
                     request for it pending, and `permits.deadlock_freedom` requires each to be \
                     granted eventually"
                ),
            })
        };
        Ok(Self {
            per_agent: limit("max_per_agent", per_agent)?,
            per_pool: limit("max_per_pool", per_pool)?,
        })
    }

    #[must_use]
    pub fn defaulted(max_parallel: u32) -> Self {
        let limit = NonZeroU32::new(max_parallel).unwrap_or(NonZeroU32::MIN);
        Self {
            per_agent: limit,
            per_pool: limit,
        }
    }

    #[must_use]
    pub const fn per_agent(self) -> u32 {
        self.per_agent.get()
    }

    #[must_use]
    pub const fn per_pool(self) -> u32 {
        self.per_pool.get()
    }
}

#[derive(Debug)]
pub struct SlotTable {
    limits: SlotLimits,
    holders: BTreeMap<InvocationId, SlotPair>,
    pending: VecDeque<(InvocationId, SlotPair)>,
    granted: u32,
    released: u32,
}

impl Default for SlotTable {
    fn default() -> Self {
        Self::with_limits(SlotLimits::WIDTH_ONE)
    }
}

impl SlotTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn with_limits(limits: SlotLimits) -> Self {
        Self {
            limits,
            holders: BTreeMap::new(),
            pending: VecDeque::new(),
            granted: 0,
            released: 0,
        }
    }

    #[must_use]
    pub const fn limits(&self) -> SlotLimits {
        self.limits
    }

    fn enqueue(&mut self, invocation: &InvocationId, pair: SlotPair) -> Vec<InvocationId> {
        self.pending.push_back((invocation.clone(), pair));
        self.pump()
    }

    fn release(&mut self, invocation: &InvocationId) -> Option<Vec<InvocationId>> {
        self.holders.remove(invocation)?;
        self.released = self.released.saturating_add(1);
        Some(self.pump())
    }

    fn withdraw(&mut self, invocation: &InvocationId) -> Option<Vec<InvocationId>> {
        let index = self
            .pending
            .iter()
            .position(|(waiting, _)| waiting == invocation)?;
        self.pending.remove(index)?;
        Some(self.pump())
    }

    fn withdraw_all(&mut self) -> Vec<InvocationId> {
        self.pending
            .drain(..)
            .map(|(invocation, _)| invocation)
            .collect()
    }

    fn pump(&mut self) -> Vec<InvocationId> {
        let mut granted = Vec::new();
        while let Some((_, pair)) = self.pending.front() {
            if !self.fits(pair, None) {
                break;
            }
            if let Some((invocation, pair)) = self.pending.pop_front() {
                self.grant(&invocation, pair);
                granted.push(invocation);
            }
        }
        let Some(reserved) = self.pending.front().map(|(_, pair)| pair.clone()) else {
            return granted;
        };
        let mut index = 1;
        while let Some((_, pair)) = self.pending.get(index) {
            if self.fits(pair, Some(&reserved)) {
                if let Some((invocation, pair)) = self.pending.remove(index) {
                    self.grant(&invocation, pair);
                    granted.push(invocation);
                    continue;
                }
            }
            index += 1;
        }
        granted
    }

    fn grant(&mut self, invocation: &InvocationId, pair: SlotPair) {
        self.holders.insert(invocation.clone(), pair);
        self.granted = self.granted.saturating_add(1);
    }

    fn fits(&self, pair: &SlotPair, reserved: Option<&SlotPair>) -> bool {
        let agent_reserved = reserved.is_some_and(|head| head.agent == pair.agent);
        let agent_fits = room(
            self.held_by_agent(&pair.agent),
            agent_reserved,
            self.limits.per_agent(),
        );
        let pool_fits = pair.pool.as_deref().is_none_or(|pool| {
            let pool_reserved = reserved.and_then(|head| head.pool.as_deref()) == Some(pool);
            room(
                self.held_by_pool(pool),
                pool_reserved,
                self.limits.per_pool(),
            )
        });
        agent_fits && pool_fits
    }

    #[must_use]
    pub fn holds(&self, invocation: &InvocationId) -> bool {
        self.holders.contains_key(invocation)
    }

    #[must_use]
    pub fn pair_of(&self, invocation: &InvocationId) -> Option<&SlotPair> {
        self.holders.get(invocation)
    }

    #[must_use]
    pub fn is_pending(&self, invocation: &InvocationId) -> bool {
        self.pending
            .iter()
            .any(|(waiting, _)| waiting == invocation)
    }

    #[must_use]
    pub fn head(&self) -> Option<&InvocationId> {
        self.pending.front().map(|(invocation, _)| invocation)
    }

    #[must_use]
    pub fn reserved(&self) -> Option<&SlotPair> {
        self.pending.front().map(|(_, pair)| pair)
    }

    #[must_use]
    pub fn holders(&self) -> Vec<&InvocationId> {
        self.holders.keys().collect()
    }

    #[must_use]
    pub fn pending(&self) -> Vec<&InvocationId> {
        self.pending
            .iter()
            .map(|(invocation, _)| invocation)
            .collect()
    }

    #[must_use]
    pub fn held_by_agent(&self, agent: &str) -> u32 {
        count(self.holders.values().filter(|held| held.agent == agent))
    }

    #[must_use]
    pub fn held_by_pool(&self, pool: &str) -> u32 {
        count(
            self.holders
                .values()
                .filter(|held| held.pool.as_deref() == Some(pool)),
        )
    }

    #[must_use]
    pub fn blocking(&self, pair: &SlotPair) -> Vec<&InvocationId> {
        self.holders
            .iter()
            .filter(|(_, held)| {
                held.agent == pair.agent
                    || (held.pool.is_some() && held.pool.as_deref() == pair.pool.as_deref())
            })
            .map(|(invocation, _)| invocation)
            .collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.holders.is_empty() && self.pending.is_empty()
    }

    #[must_use]
    pub const fn granted(&self) -> u32 {
        self.granted
    }

    #[must_use]
    pub const fn released(&self) -> u32 {
        self.released
    }

    #[must_use]
    pub fn balances(&self) -> bool {
        self.granted == self.released && self.is_empty()
    }
}

fn room(held: u32, reserved: bool, limit: u32) -> bool {
    held.saturating_add(u32::from(reserved)) < limit
}

fn count<T>(items: impl Iterator<Item = T>) -> u32 {
    u32::try_from(items.count()).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReservationKind {
    Dispatch,
    Retry,
    Integration,
}

impl ReservationKind {
    #[must_use]
    pub const fn entitlements(self) -> u32 {
        match self {
            Self::Dispatch | Self::Retry => 1,
            Self::Integration => 2,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::Retry => "retry",
            Self::Integration => "integration",
        }
    }

    #[must_use]
    pub const fn holds_merge(self) -> bool {
        matches!(self, Self::Integration)
    }
}

#[derive(Debug, Default)]
pub struct Reservations {
    held: BTreeMap<TaskKey, ReservationKind>,
    taken_ever: BTreeSet<(TaskKey, ReservationKind)>,
    peak: usize,
    taken: u32,
    converted: u32,
    cancelled: u32,
    duplicates: u32,
}

impl Reservations {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {
        if let Some((held, held_kind)) = self.held.iter().next() {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "the {} reservation for task {held} is already held; the sequential \
                     substrate asserts at most one, and the {} reservation for task {key} \
                     would be a second",
                    held_kind.name(),
                    kind.name()
                ),
            });
        }
        self.hold(key, kind);
        Ok(())
    }

    pub fn reserve(
        &mut self,
        key: TaskKey,
        kind: ReservationKind,
        derived: &Entitlements,
    ) -> Result<(), UpstrokeError> {
        if let Some(held_kind) = self.held.get(&key) {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "task {key} already holds its {} reservation, and the {} reservation asked \
                     for it would be a second for one selected task: a provisional reservation \
                     is one per selected task or candidate",
                    held_kind.name(),
                    kind.name()
                ),
            });
        }
        if derived.poisoned() {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "the {} reservation for task {key} was asked of a poisoned fold: an append \
                     returned an error, and `permits.provisional_reservations` cancels every \
                     reservation on a poisoned fold rather than taking one",
                    kind.name()
                ),
            });
        }
        let pipeline = derived
            .pipeline_held()
            .saturating_add(self.pipeline_outstanding());
        let limit = usize::try_from(derived.max_parallel()).unwrap_or(usize::MAX);
        if pipeline >= limit {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "the {} reservation for task {key} is not reservable: the fold holds {} \
                     pipeline entitlement(s) and provisional reservations {} more, {pipeline} of \
                     max_parallel = {}. A reservation is taken only when the derived count plus \
                     the outstanding reservations permits, and is never awaited",
                    kind.name(),
                    derived.pipeline_held(),
                    self.pipeline_outstanding(),
                    derived.max_parallel()
                ),
            });
        }
        if kind.holds_merge()
            && derived
                .merge_held()
                .saturating_add(self.merge_outstanding())
                >= MERGE_ENTITLEMENTS
        {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "an integration reservation for task {key} is not reservable: the one merge \
                     entitlement is held ({} by the fold, {} by a provisional reservation), and \
                     it is never awaited",
                    derived.merge_held(),
                    self.merge_outstanding()
                ),
            });
        }
        self.hold(key, kind);
        Ok(())
    }

    fn hold(&mut self, key: TaskKey, kind: ReservationKind) {
        self.held.insert(key, kind);
        self.taken_ever.insert((key, kind));
        self.taken = self.taken.saturating_add(1);
        self.peak = self.peak.max(self.held.len());
    }

    #[must_use]
    pub const fn peak(&self) -> usize {
        self.peak
    }

    pub fn convert(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {
        if self.settle(key, kind, "converted")? {
            self.converted = self.converted.saturating_add(1);
        }
        Ok(())
    }

    pub fn cancel(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {
        if self.settle(key, kind, "cancelled")? {
            self.cancelled = self.cancelled.saturating_add(1);
        }
        Ok(())
    }

    pub fn cancel_all(&mut self) -> usize {
        let outstanding = self.held.len();
        self.held.clear();
        self.cancelled = self
            .cancelled
            .saturating_add(u32::try_from(outstanding).unwrap_or(u32::MAX));
        outstanding
    }

    pub fn cancel_any(&mut self) -> bool {
        self.cancel_all() > 0
    }

    fn settle(
        &mut self,
        key: TaskKey,
        kind: ReservationKind,
        verb: &str,
    ) -> Result<bool, UpstrokeError> {
        match self.held.get(&key) {
            Some(held_kind) if *held_kind == kind => {
                self.held.remove(&key);
                Ok(true)
            }
            Some(held_kind) => Err(UpstrokeError::Refused {
                message: format!(
                    "the {} reservation for task {key} was {verb}, but the one task {key} holds \
                     is its {} reservation",
                    kind.name(),
                    held_kind.name()
                ),
            }),
            None if self.taken_ever.contains(&(key, kind)) => {
                self.duplicates = self.duplicates.saturating_add(1);
                Ok(false)
            }
            None => Err(UpstrokeError::Refused {
                message: format!(
                    "the {} reservation for task {key} was {verb}, and none was ever taken for it",
                    kind.name()
                ),
            }),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    #[must_use]
    pub fn held(&self, key: TaskKey) -> Option<ReservationKind> {
        self.held.get(&key).copied()
    }

    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.held.len()
    }

    #[must_use]
    pub fn pipeline_outstanding(&self) -> usize {
        self.held.len()
    }

    #[must_use]
    pub fn merge_outstanding(&self) -> usize {
        self.held.values().filter(|kind| kind.holds_merge()).count()
    }

    #[must_use]
    pub fn entitlements_held(&self) -> u32 {
        self.held
            .values()
            .map(|kind| kind.entitlements())
            .fold(0, u32::saturating_add)
    }

    #[must_use]
    pub const fn taken(&self) -> u32 {
        self.taken
    }

    #[must_use]
    pub const fn converted(&self) -> u32 {
        self.converted
    }

    #[must_use]
    pub const fn cancelled(&self) -> u32 {
        self.cancelled
    }

    #[must_use]
    pub const fn duplicates(&self) -> u32 {
        self.duplicates
    }

    #[must_use]
    pub fn balances(&self) -> bool {
        self.taken == self.converted.saturating_add(self.cancelled) && self.held.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Runnable,
    Granted,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Registration {
    Pending,
    Running,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Settlement {
    Completed,
    Cancelled,
}

#[derive(Debug)]
struct Entry {
    invocation: InvocationId,
    state: Registration,
}

#[derive(Debug, Default)]
pub struct InvocationLedger {
    entries: BTreeMap<String, Entry>,
    duplicates: u32,
    slots: SlotTable,
}

impl InvocationLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_limits(limits: SlotLimits) -> Self {
        Self {
            entries: BTreeMap::new(),
            duplicates: 0,
            slots: SlotTable::with_limits(limits),
        }
    }

    pub fn register(&mut self, invocation: &InvocationId) -> Result<(), UpstrokeError> {
        if is_slotted(invocation) {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "`{invocation}` is slotted and registers with its pair: \
                     `permits.agent_pool_slots` gives every agent CLI invocation its atomic \
                     `{{agent, pool?}}` pair"
                ),
            });
        }
        self.open(invocation, Registration::Running)
    }

    pub fn register_slotted(
        &mut self,
        invocation: &InvocationId,
        pair: SlotPair,
        standing: &Standing,
    ) -> Result<Admission, UpstrokeError> {
        if !is_slotted(invocation) {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "`{invocation}` is a gate or the shell probe and acquires no slot: \
                     `permits.agent_pool_slots` excludes both by name"
                ),
            });
        }
        standing.admits(invocation)?;
        self.open(invocation, Registration::Pending)?;
        let granted = self.slots.enqueue(invocation, pair);
        self.run_granted(&granted);
        Ok(if self.slots.holds(invocation) {
            Admission::Granted
        } else {
            Admission::Pending
        })
    }

    pub fn register_at_once(
        &mut self,
        invocation: &InvocationId,
        slots: Option<(SlotPair, &Standing)>,
    ) -> Result<(), UpstrokeError> {
        let Some((pair, standing)) = slots else {
            return self.register(invocation);
        };
        let wanted = pair.clone();
        if self.register_slotted(invocation, pair, standing)? != Admission::Pending {
            return Ok(());
        }
        let mut waits_on: Vec<String> = self
            .slots
            .blocking(&wanted)
            .into_iter()
            .map(|holder| format!("held by `{holder}`"))
            .collect();
        if let Some(head) = self.slots.head().filter(|head| *head != invocation) {
            waits_on.push(format!("reserved by the pending head `{head}`"));
        }
        let withdrawn = self.cancel(invocation).map(drop);
        Err(UpstrokeError::Refused {
            message: format!(
                "`{invocation}` would wait for its pair {wanted} ({}). INV-18: \"The coordinator \
                 never blocks on an entitlement, provisional reservation, or slot\", and this is \
                 the synchronous substrate, where the coordinator runs the invocation itself, so \
                 a pair it cannot be granted at once is a hold a caller leaked. The request was \
                 withdrawn and nothing was spawned",
                waits_on.join("; ")
            ),
        }
        .with_cleanup(withdrawn))
    }

    fn open(
        &mut self,
        invocation: &InvocationId,
        state: Registration,
    ) -> Result<(), UpstrokeError> {
        let key = invocation.render();
        if self.entries.contains_key(&key) {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "`{key}` is already registered: two processes would share one identity"
                ),
            });
        }
        self.entries.insert(
            key,
            Entry {
                invocation: invocation.clone(),
                state,
            },
        );
        Ok(())
    }

    fn run_granted(&mut self, granted: &[InvocationId]) {
        for invocation in granted {
            if let Some(entry) = self.entries.get_mut(&invocation.render()) {
                entry.state = Registration::Running;
            }
        }
    }

    pub fn complete(
        &mut self,
        invocation: &InvocationId,
    ) -> Result<Vec<InvocationId>, UpstrokeError> {
        self.settle(invocation, Settlement::Completed)
    }

    pub fn cancel(
        &mut self,
        invocation: &InvocationId,
    ) -> Result<Vec<InvocationId>, UpstrokeError> {
        self.settle(invocation, Settlement::Cancelled)
    }

    fn settle(
        &mut self,
        invocation: &InvocationId,
        to: Settlement,
    ) -> Result<Vec<InvocationId>, UpstrokeError> {
        let key = invocation.render();
        let Some(entry) = self.entries.get_mut(&key) else {
            return Err(UpstrokeError::Refused {
                message: format!("`{key}` was settled without ever being registered"),
            });
        };
        let granted = match (entry.state, to) {
            (Registration::Completed | Registration::Cancelled, _) => {
                self.duplicates = self.duplicates.saturating_add(1);
                return Ok(Vec::new());
            }
            (Registration::Pending, Settlement::Completed) => {
                return Err(UpstrokeError::Refused {
                    message: format!(
                        "`{key}` is still waiting for its slot pair, so no process of it ran and \
                         it cannot complete; `permits.protocol` removes a pending request by \
                         cancelling it"
                    ),
                });
            }
            (Registration::Pending, Settlement::Cancelled) => {
                entry.state = Registration::Cancelled;
                self.slots.withdraw(invocation)
            }
            (Registration::Running, _) => {
                entry.state = match to {
                    Settlement::Completed => Registration::Completed,
                    Settlement::Cancelled => Registration::Cancelled,
                };
                self.slots.release(invocation)
            }
        }
        .unwrap_or_default();
        self.run_granted(&granted);
        Ok(granted)
    }

    pub fn cancel_all_running(&mut self) -> usize {
        let withdrawn = self.withdraw_pending();
        let running: Vec<InvocationId> = self
            .entries
            .values()
            .filter(|entry| entry.state == Registration::Running)
            .map(|entry| entry.invocation.clone())
            .collect();
        for invocation in &running {
            if let Some(entry) = self.entries.get_mut(&invocation.render()) {
                entry.state = Registration::Cancelled;
            }
            drop(self.slots.release(invocation));
        }
        withdrawn.saturating_add(running.len())
    }

    pub fn withdraw_pending(&mut self) -> usize {
        let withdrawn = self.slots.withdraw_all();
        for invocation in &withdrawn {
            if let Some(entry) = self.entries.get_mut(&invocation.render()) {
                entry.state = Registration::Cancelled;
            }
        }
        withdrawn.len()
    }

    #[must_use]
    pub const fn duplicates(&self) -> u32 {
        self.duplicates
    }

    #[must_use]
    pub fn completed(&self) -> usize {
        self.count(Registration::Completed)
    }

    #[must_use]
    pub fn cancelled(&self) -> usize {
        self.count(Registration::Cancelled)
    }

    #[must_use]
    pub fn registered(&self) -> usize {
        self.entries.len()
    }

    fn count(&self, state: Registration) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.state == state)
            .count()
    }

    #[must_use]
    pub fn balances(&self) -> bool {
        self.entries
            .values()
            .all(|entry| !matches!(entry.state, Registration::Pending | Registration::Running))
            && self.slots.balances()
    }

    #[must_use]
    pub fn running(&self) -> Vec<&str> {
        self.in_state(Registration::Running)
    }

    #[must_use]
    pub fn settled(&self, invocation: &InvocationId) -> bool {
        self.entries.get(&invocation.render()).is_some_and(|entry| {
            matches!(
                entry.state,
                Registration::Completed | Registration::Cancelled
            )
        })
    }

    #[must_use]
    pub fn pending(&self) -> Vec<&str> {
        self.in_state(Registration::Pending)
    }

    fn in_state(&self, state: Registration) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.state == state)
            .map(|(key, _)| key.as_str())
            .collect()
    }

    #[must_use]
    pub const fn slots(&self) -> &SlotTable {
        &self.slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: TaskKey = TaskKey(3);
    const OTHER: TaskKey = TaskKey(4);
    const GEN: GenerationId = GenerationId(2);

    fn ids(attempt: u32) -> AttemptIdentities {
        AttemptIdentities::new(KEY, GEN, AttemptNumber(attempt))
    }

    fn pair(agent: &str) -> SlotPair {
        SlotPair {
            agent: agent.to_owned(),
            pool: None,
        }
    }

    #[test]
    fn every_invocation_of_an_attempt_is_distinct_and_a_retry_reuses_none_of_them() {
        let first = ids(1);
        let retry = ids(2);

        let of = |a: &AttemptIdentities| {
            let mut v = vec![a.worker()];
            for n in 0..3 {
                v.push(a.gate(n, 0));
                v.push(a.gate(n, 1));
                v.push(a.review_pass(n, 0));
                v.push(a.review_reask(n, 0));
            }
            v.into_iter().map(|id| id.render()).collect::<Vec<_>>()
        };

        let a = of(&first);
        let b = of(&retry);
        let unique: std::collections::BTreeSet<&String> = a.iter().chain(b.iter()).collect();
        assert_eq!(
            unique.len(),
            a.len() + b.len(),
            "two invocations share an identity: {a:?} {b:?}"
        );

        assert_ne!(first.gate(0, 0), first.gate(0, 1));
        assert_ne!(first.gate(0, 1), first.gate(1, 0));
    }

    #[test]
    fn an_identity_is_a_pure_function_of_its_tuple() {
        assert_eq!(ids(1).worker(), ids(1).worker());
        assert_eq!(ids(1).gate(2, 3), ids(1).gate(2, 3));
        assert_eq!(
            PreflightIdentities::shell(0).expect("the shell probe"),
            PreflightIdentities::shell(0).expect("the shell probe")
        );
        assert_eq!(
            PreflightIdentities::agent("claude", 0).expect("an agent probe"),
            PreflightIdentities::agent("claude", 0).expect("an agent probe")
        );
    }

    #[test]
    fn a_sequence_has_no_worker_and_shares_no_identity_with_an_attempt() {
        let seq = SequenceIdentities::new(SequenceId(1));
        let attempt = ids(1);
        let rendered: std::collections::BTreeSet<String> = [
            seq.gate(0, 0).render(),
            seq.review_pass(0, 0).render(),
            seq.review_reask(0, 0).render(),
            attempt.gate(0, 0).render(),
            attempt.review_pass(0, 0).render(),
            attempt.review_reask(0, 0).render(),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            rendered.len(),
            6,
            "a sequence identity collided with an attempt's"
        );
    }

    #[test]
    fn a_probe_identity_carries_no_epoch_and_therefore_repeats_across_incarnations() {
        let first = PreflightIdentities::agent("claude", 0).expect("an agent probe");
        let second = PreflightIdentities::agent("claude", 0).expect("an agent probe");
        assert_eq!(
            first, second,
            "probe identities repeat by construction; the container name's \
             incarnation component is what separates two epochs' probes"
        );
        assert!(!first.render().contains("01KZT"), "{}", first.render());
    }

    #[test]
    fn an_agent_probe_refuses_a_name_that_is_not_a_safe_component() {
        for hostile in ["../escape", "has space", "semi;colon", ""] {
            assert!(
                PreflightIdentities::agent(hostile, 0).is_err(),
                "`{hostile}` was accepted as an agent probe target"
            );
        }
        assert!(PreflightIdentities::agent("claude-code_1", 0).is_ok());
    }

    fn probe(agent: &str, ordinal: u32) -> InvocationId {
        PreflightIdentities::agent(agent, ordinal).expect("an agent probe")
    }

    fn pooled(agent: &str, pool: &str) -> SlotPair {
        SlotPair {
            agent: agent.to_owned(),
            pool: Some(pool.to_owned()),
        }
    }

    fn limits(per_agent: u32, per_pool: u32) -> SlotLimits {
        SlotLimits::new(per_agent, per_pool).expect("limits of at least one")
    }

    fn slotted(ledger: &mut InvocationLedger, id: &InvocationId, pair: SlotPair) -> Admission {
        ledger
            .register_slotted(id, pair, &Standing::preflight())
            .unwrap_or_else(|error| panic!("`{id}` registers with its pair: {error}"))
    }

    fn settled(ledger: &mut InvocationLedger, id: &InvocationId) -> Vec<InvocationId> {
        ledger
            .complete(id)
            .unwrap_or_else(|error| panic!("`{id}` completes: {error}"))
    }

    #[test]
    fn the_synchronous_substrate_refuses_a_pair_it_cannot_grant_at_once() {
        let mut ledger = InvocationLedger::new();
        assert!(
            ledger.slots().is_empty(),
            "a process starts with an empty slot table"
        );
        let holder = probe("claude", 0);
        let later = probe("claude", 1);
        ledger
            .register_at_once(&holder, Some((pair("claude"), &Standing::preflight())))
            .expect("the first pair");
        assert!(ledger.slots().holds(&holder));

        let refused = ledger
            .register_at_once(&later, Some((pair("claude"), &Standing::preflight())))
            .expect_err("a pair a leaked hold keeps cannot be granted at once");
        let message = refused.to_string();
        assert!(
            message.contains(
                "The coordinator never blocks on an entitlement, provisional reservation, or slot"
            ),
            "the refusal carries INV-18's sentence: {message}"
        );
        assert!(
            message.contains(&holder.render()),
            "and names the hold: {message}"
        );
        assert!(
            ledger.slots().holds(&holder),
            "the refusal must not disturb the held pair"
        );
        assert!(
            ledger.slots().pending().is_empty(),
            "the refused request was withdrawn rather than left waiting"
        );
        assert_eq!(
            (ledger.cancelled(), ledger.duplicates()),
            (1, 0),
            "registered and withdrawn exactly once"
        );

        settled(&mut ledger, &holder);
        let next = probe("claude", 2);
        ledger
            .register_at_once(&next, Some((pair("claude"), &Standing::preflight())))
            .expect("once the hold is released the next pair is granted at once");
        settled(&mut ledger, &next);
        assert!(ledger.balances());
    }

    #[test]
    fn a_gate_and_the_shell_probe_are_refused_a_slot_pair() {
        let attempt = ids(1);
        let seq = SequenceIdentities::new(SequenceId(1));

        for (label, id) in [
            ("an attempt's gate", attempt.gate(0, 0)),
            ("a sequence's gate", seq.gate(0, 0)),
            (
                "the shell probe",
                PreflightIdentities::shell(0).expect("the shell probe"),
            ),
        ] {
            assert!(!is_slotted(&id), "{label} was classified as slotted");
            let mut ledger = InvocationLedger::new();
            let refused = ledger
                .register_slotted(&id, pair("claude"), &Standing::preflight())
                .expect_err("{label} was given a slot pair");
            assert!(
                refused.to_string().contains("acquires no slot"),
                "{label}: {refused}"
            );
            assert!(ledger.slots().is_empty(), "{label} left a pair held");
            ledger.register(&id).expect("and registers without one");
            assert_eq!(ledger.slots().granted(), 0, "{label} took a pair");
            settled(&mut ledger, &id);
            assert!(ledger.balances());
        }

        for (label, id) in [
            ("the worker", attempt.worker()),
            ("a review pass", attempt.review_pass(0, 0)),
            ("a re-ask", attempt.review_reask(0, 0)),
            ("an agent probe", probe("claude", 0)),
        ] {
            assert!(is_slotted(&id), "{label} was classified as non-slotted");
            let mut ledger = InvocationLedger::new();
            let refused = ledger
                .register(&id)
                .expect_err("{label} registered without its pair");
            assert!(
                refused.to_string().contains("registers with its pair"),
                "{label}: {refused}"
            );
            assert_eq!(ledger.registered(), 0, "{label}'s refusal registered it");
        }

        let agent_probe = probe("claude", 0);
        let mut ledger = InvocationLedger::new();
        assert_eq!(
            slotted(&mut ledger, &agent_probe, pair("claude")),
            Admission::Granted
        );
        assert_eq!(
            ledger
                .slots()
                .pair_of(&agent_probe)
                .map(|p| p.agent.as_str()),
            Some("claude"),
            "the agent probe held a pair the table cannot name"
        );
        settled(&mut ledger, &agent_probe);
        assert!(ledger.balances());
    }

    #[test]
    fn waiting_pairs_are_granted_in_arrival_order_as_their_slots_are_released() {
        let mut ledger = InvocationLedger::new();
        let (first, second, third) = (probe("claude", 0), probe("claude", 1), probe("claude", 2));
        assert_eq!(
            slotted(&mut ledger, &first, pair("claude")),
            Admission::Granted
        );
        assert_eq!(
            slotted(&mut ledger, &second, pair("claude")),
            Admission::Pending
        );
        assert_eq!(
            slotted(&mut ledger, &third, pair("claude")),
            Admission::Pending
        );
        assert_eq!(ledger.slots().head(), Some(&second));
        assert_eq!(ledger.slots().pending(), vec![&second, &third]);

        assert_eq!(
            settled(&mut ledger, &first),
            vec![second.clone()],
            "the release grants the head, not a later arrival"
        );
        assert_eq!(ledger.running(), vec![second.render()]);
        assert_eq!(ledger.slots().head(), Some(&third));
        assert_eq!(settled(&mut ledger, &second), vec![third.clone()]);
        assert_eq!(settled(&mut ledger, &third), Vec::new());
        assert!(ledger.balances());
        assert_eq!(
            (ledger.slots().granted(), ledger.slots().released()),
            (3, 3)
        );
    }

    #[test]
    fn a_waiting_head_reserves_every_slot_it_needs_so_no_later_request_starves_it() {
        let mut ledger = InvocationLedger::with_limits(limits(1, 1));
        let agent_holder = probe("claude", 0);
        let pool_holder = probe("codex", 0);
        assert_eq!(
            slotted(&mut ledger, &agent_holder, pooled("claude", "own")),
            Admission::Granted
        );
        assert_eq!(
            slotted(&mut ledger, &pool_holder, pooled("codex", "shared")),
            Admission::Granted
        );

        let head = probe("claude", 1);
        let later = probe("copilot", 0);
        assert_eq!(
            slotted(&mut ledger, &head, pooled("claude", "shared")),
            Admission::Pending,
            "the head waits for its agent and its pool"
        );
        assert_eq!(
            slotted(&mut ledger, &later, pooled("copilot", "shared")),
            Admission::Pending
        );
        assert_eq!(ledger.slots().reserved(), Some(&pooled("claude", "shared")));

        assert_eq!(
            settled(&mut ledger, &pool_holder),
            Vec::new(),
            "the pool slot the head waits for is reserved, so the later request that fits it now \
             is not granted it"
        );
        assert!(ledger.slots().is_pending(&later));

        assert_eq!(
            settled(&mut ledger, &agent_holder),
            vec![head.clone()],
            "once its agent frees the head takes both slots"
        );
        assert_eq!(ledger.slots().head(), Some(&later));
        assert_eq!(settled(&mut ledger, &head), vec![later.clone()]);
        assert_eq!(settled(&mut ledger, &later), Vec::new());
        assert!(ledger.balances());
    }

    #[test]
    fn a_later_request_runs_past_a_waiting_head_only_when_it_takes_nothing_the_head_needs() {
        let mut ledger = InvocationLedger::with_limits(limits(1, 2));
        let running = probe("claude", 0);
        assert_eq!(
            slotted(&mut ledger, &running, pooled("claude", "shared")),
            Admission::Granted
        );
        let head = probe("claude", 1);
        assert_eq!(
            slotted(&mut ledger, &head, pooled("claude", "shared")),
            Admission::Pending,
            "the head waits for its agent"
        );

        let wants_the_heads_pool = probe("codex", 0);
        assert_eq!(
            slotted(
                &mut ledger,
                &wants_the_heads_pool,
                pooled("codex", "shared")
            ),
            Admission::Pending,
            "one of the pool's two slots is free and the head reserves it"
        );
        let wants_the_heads_agent = probe("claude", 2);
        assert_eq!(
            slotted(&mut ledger, &wants_the_heads_agent, pair("claude")),
            Admission::Pending
        );
        let disjoint = probe("copilot", 0);
        assert_eq!(
            slotted(&mut ledger, &disjoint, pooled("copilot", "own")),
            Admission::Granted,
            "a request taking nothing the head needs is backfilled past it"
        );
        let unpooled = probe("gemini", 0);
        assert_eq!(
            slotted(&mut ledger, &unpooled, pair("gemini")),
            Admission::Granted,
            "an agent without a pool takes its agent slot only"
        );
        assert_eq!(ledger.slots().held_by_pool("shared"), 1);

        assert_eq!(
            settled(&mut ledger, &running),
            vec![head.clone(), wants_the_heads_pool.clone()],
            "the release grants the head first, then the next arrival that now fits"
        );
        assert_eq!(ledger.slots().head(), Some(&wants_the_heads_agent));
        assert_eq!(
            settled(&mut ledger, &head),
            vec![wants_the_heads_agent.clone()]
        );
        for id in [
            &wants_the_heads_pool,
            &wants_the_heads_agent,
            &disjoint,
            &unpooled,
        ] {
            settled(&mut ledger, id);
        }
        assert!(ledger.balances());
    }

    #[test]
    fn a_cancelled_head_recomputes_the_reservation_and_a_duplicate_cancel_is_counted() {
        let mut ledger = InvocationLedger::with_limits(limits(1, 1));
        let agent_holder = probe("claude", 0);
        let pool_holder = probe("codex", 0);
        slotted(&mut ledger, &agent_holder, pooled("claude", "own"));
        slotted(&mut ledger, &pool_holder, pooled("codex", "shared"));
        let head = probe("claude", 1);
        let later = probe("copilot", 0);
        slotted(&mut ledger, &head, pooled("claude", "shared"));
        slotted(&mut ledger, &later, pooled("copilot", "shared"));
        settled(&mut ledger, &pool_holder);
        assert!(
            ledger.slots().is_pending(&later),
            "the head's reservation holds the freed pool slot"
        );

        assert_eq!(
            ledger.cancel(&head).expect("the head is withdrawn"),
            vec![later.clone()],
            "withdrawing the head moves the reservation, and the new head fits"
        );
        assert_eq!(ledger.slots().head(), None);
        assert!(ledger.slots().holds(&later));

        assert_eq!(
            ledger.cancel(&head).expect("a duplicate is not an error"),
            Vec::new(),
            "a duplicate cancel grants nothing"
        );
        assert_eq!(ledger.duplicates(), 1, "and is counted");
        assert!(
            ledger.slots().holds(&later) && ledger.slots().holds(&agent_holder),
            "and releases nothing"
        );
        assert_eq!(ledger.slots().held_by_pool("shared"), 1);

        settled(&mut ledger, &agent_holder);
        settled(&mut ledger, &later);
        assert!(ledger.balances());
    }

    #[test]
    fn a_pending_invocation_cannot_complete_and_cancelling_it_withdraws_it() {
        let mut ledger = InvocationLedger::new();
        let holder = probe("claude", 0);
        let waiting = probe("claude", 1);
        slotted(&mut ledger, &holder, pair("claude"));
        assert_eq!(
            slotted(&mut ledger, &waiting, pair("claude")),
            Admission::Pending
        );
        assert_eq!(ledger.pending(), vec![waiting.render()]);

        let refused = ledger
            .complete(&waiting)
            .expect_err("no process of a waiting request ran");
        assert!(
            refused
                .to_string()
                .contains("still waiting for its slot pair"),
            "{refused}"
        );
        assert!(
            ledger.slots().is_pending(&waiting),
            "the refusal withdrew it"
        );

        assert_eq!(ledger.cancel(&waiting).expect("withdrawn"), Vec::new());
        assert!(!ledger.slots().is_pending(&waiting));
        settled(&mut ledger, &holder);
        assert!(ledger.balances());
        assert_eq!(
            (ledger.completed(), ledger.cancelled()),
            (1, 1),
            "one ran, one was withdrawn before it was granted"
        );
    }

    #[test]
    fn a_duplicate_settlement_releases_no_pair_twice_and_no_count_goes_negative() {
        let mut ledger = InvocationLedger::with_limits(limits(2, 2));
        let first = probe("claude", 0);
        let second = probe("claude", 1);
        slotted(&mut ledger, &first, pooled("claude", "shared"));
        slotted(&mut ledger, &second, pooled("claude", "shared"));
        assert_eq!(ledger.slots().held_by_agent("claude"), 2);

        settled(&mut ledger, &first);
        for duplicate in 0..3 {
            assert_eq!(
                ledger.complete(&first).expect("a duplicate completion"),
                Vec::new()
            );
            assert_eq!(ledger.cancel(&first).expect("a late cancel"), Vec::new());
            assert_eq!(
                ledger.slots().held_by_agent("claude"),
                1,
                "duplicate {duplicate} released the second invocation's slot"
            );
            assert!(ledger.slots().holds(&second));
        }
        assert_eq!(ledger.duplicates(), 6);
        assert_eq!(ledger.slots().released(), 1, "released exactly once");

        settled(&mut ledger, &second);
        assert_eq!(
            (
                ledger.slots().held_by_agent("claude"),
                ledger.slots().held_by_pool("shared")
            ),
            (0, 0)
        );
        assert!(ledger.balances());
    }

    #[test]
    fn slot_limits_of_zero_are_refused_and_the_default_follows_max_parallel() {
        assert!(SlotLimits::new(0, 1).is_err());
        assert!(SlotLimits::new(1, 0).is_err());
        let three = SlotLimits::defaulted(3);
        assert_eq!((three.per_agent(), three.per_pool()), (3, 3));
        assert_eq!(SlotLimits::defaulted(1), SlotLimits::WIDTH_ONE);
        assert_eq!(
            SlotLimits::defaulted(0),
            SlotLimits::WIDTH_ONE,
            "the fold refuses max_parallel = 0 at run_started, so the floor never binds a run"
        );
        assert_eq!(SlotTable::new().limits(), SlotLimits::WIDTH_ONE);
    }

    #[test]
    fn a_reservation_is_asserted_singly_and_settles_exactly_once() {
        let mut r = Reservations::new();
        assert!(r.is_empty(), "a process starts with no reservation");
        assert!(r.balances());

        r.take(KEY, ReservationKind::Dispatch).expect("the first");
        assert_eq!(r.entitlements_held(), 1, "dispatch holds {{pipeline}}");
        assert!(
            r.take(OTHER, ReservationKind::Dispatch).is_err(),
            "the sequential form took a second reservation"
        );
        assert!(!r.balances(), "an outstanding reservation cannot balance");

        r.convert(KEY, ReservationKind::Dispatch)
            .expect("converted at its append");
        assert!(r.balances());
        r.convert(KEY, ReservationKind::Dispatch)
            .expect("a duplicate conversion is ignored, not refused");
        r.cancel(KEY, ReservationKind::Dispatch)
            .expect("and so is a stale cancel");
        assert_eq!(
            (r.converted(), r.cancelled(), r.duplicates()),
            (1, 0, 2),
            "settled once; the two later operations were counted and released nothing"
        );
        assert!(r.balances());
    }

    #[test]
    fn a_reservation_settled_under_the_wrong_name_is_refused() {
        let mut r = Reservations::new();
        r.take(KEY, ReservationKind::Retry)
            .expect("a retry reservation");

        assert!(
            r.convert(OTHER, ReservationKind::Retry).is_err(),
            "a task that never reserved"
        );
        assert!(
            r.convert(KEY, ReservationKind::Dispatch).is_err(),
            "wrong kind"
        );
        assert!(
            !r.is_empty(),
            "a refused settlement must not release the hold"
        );
        assert_eq!(r.duplicates(), 0, "a refusal is not a duplicate");

        r.cancel(KEY, ReservationKind::Retry)
            .expect("the right name");
        assert!(r.balances());
    }

    #[test]
    fn cancel_any_releases_an_unnamed_reservation_and_reports_whether_there_was_one() {
        let mut r = Reservations::new();
        assert!(!r.cancel_any(), "nothing was held");

        r.take(KEY, ReservationKind::Integration)
            .expect("an integration reservation");
        assert_eq!(
            r.entitlements_held(),
            2,
            "integration holds {{pipeline, merge}}, not {{pipeline}}"
        );
        assert!(
            r.cancel_any(),
            "the poisoned-fold path cancels what it holds"
        );
        assert!(r.balances());
        assert!(!r.cancel_any());
    }

    #[test]
    fn the_invocation_ledger_refuses_aliasing_and_counts_duplicate_settlements() {
        let mut ledger = InvocationLedger::new();
        let gate = ids(1).gate(0, 0);

        assert!(ledger.balances(), "an empty ledger balances");
        ledger.register(&gate).expect("the first registration");
        assert!(!ledger.balances(), "a running invocation is unsettled");
        assert_eq!(ledger.running(), vec![gate.render()]);

        assert!(
            ledger.register(&gate).is_err(),
            "two processes were given one identity"
        );

        ledger.complete(&gate).expect("settled");
        assert!(ledger.balances());
        assert_eq!(ledger.duplicates(), 0);

        ledger.complete(&gate).expect("a duplicate is not an error");
        ledger.cancel(&gate).expect("nor is a late cancel");
        assert_eq!(ledger.duplicates(), 2);
        assert!(ledger.balances());
    }

    #[test]
    fn settling_an_unregistered_invocation_is_refused() {
        let mut ledger = InvocationLedger::new();
        let gate = ids(1).gate(0, 0);
        assert!(ledger.complete(&gate).is_err());
        assert!(ledger.cancel(&gate).is_err());
        assert_eq!(ledger.duplicates(), 0);
    }

    #[test]
    fn cancel_all_running_settles_every_in_flight_invocation() {
        let mut ledger = InvocationLedger::new();
        let attempt = ids(1);
        let gate = attempt.gate(0, 0);
        let finished = attempt.gate(1, 0);
        let holder = probe("claude", 0);
        let waiting = probe("claude", 1);

        ledger.register(&gate).expect("registered");
        ledger.register(&finished).expect("registered");
        slotted(&mut ledger, &holder, pair("claude"));
        assert_eq!(
            slotted(&mut ledger, &waiting, pair("claude")),
            Admission::Pending
        );
        ledger
            .complete(&finished)
            .expect("one finished before the error");
        assert_eq!(ledger.running().len(), 2);

        assert_eq!(
            ledger.cancel_all_running(),
            3,
            "the two running and the one waiting are cancelled, the finished one is not re-settled"
        );
        assert!(ledger.balances());
        assert!(ledger.running().is_empty() && ledger.pending().is_empty());
        assert!(
            ledger.slots().is_empty(),
            "the held pair was released and the waiting request withdrawn without a grant"
        );
        assert_eq!(
            ledger.slots().granted(),
            1,
            "the waiting request was never granted on the way out"
        );
        assert_eq!(
            ledger.duplicates(),
            0,
            "cancelling a running invocation is not a duplicate settlement"
        );
    }
}
