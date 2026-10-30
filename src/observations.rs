//! Extended notes: `docs/internals/observations.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::topology::effects::{EffectSiteId, HookHarness, HookPhase, Injection, Observation};

pub const OBSERVATIONS_ENV: &str = "UPSTROKE_HOOK_OBSERVATIONS";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRecord {
    pub test: String,
    pub observed: Vec<Observation>,
    pub reached: Vec<Observation>,
    pub fast_sequences: Vec<FastSequenceRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastSequenceRecord {
    pub name: String,
    pub touched: Vec<String>,
}

impl ObservationRecord {
    #[must_use]
    pub fn of(test: &str, harness: &HookHarness) -> Self {
        Self {
            test: test.to_owned(),
            observed: harness.coverage().to_vec(),
            reached: harness.reached().to_vec(),
            fast_sequences: harness
                .fast_sequences()
                .iter()
                .map(|sequence| FastSequenceRecord {
                    name: sequence.name().to_owned(),
                    touched: sequence.touched().iter().map(|site| site.name()).collect(),
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.observed.is_empty() && self.reached.is_empty() && self.fast_sequences.is_empty()
    }

    pub fn merge(&mut self, other: Self) {
        merge_observations(&mut self.observed, other.observed);
        merge_observations(&mut self.reached, other.reached);
        for sequence in other.fast_sequences {
            if !self.fast_sequences.contains(&sequence) {
                self.fast_sequences.push(sequence);
            }
        }
    }

    #[must_use]
    pub fn observed(&self, site: EffectSiteId, phase: HookPhase) -> bool {
        self.observed
            .iter()
            .any(|seen| seen.site == site && seen.phase == phase && seen.count > 0)
    }

    #[must_use]
    pub fn file_name(test: &str) -> String {
        format!("{}.json", test.replace("::", "__"))
    }
}

fn merge_observations(into: &mut Vec<Observation>, from: Vec<Observation>) {
    for seen in from {
        match into
            .iter_mut()
            .find(|held| held.site == seen.site && held.phase == seen.phase)
        {
            Some(held) => held.count = held.count.max(seen.count),
            None => into.push(seen),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Exported {
    harness: Arc<Mutex<HookHarness>>,
    _on_drop: Arc<ExportOnDrop>,
}

impl Exported {
    #[must_use]
    pub fn new(harness: Arc<Mutex<HookHarness>>) -> Self {
        let on_drop = Arc::new(ExportOnDrop(Arc::clone(&harness)));
        Self {
            harness,
            _on_drop: on_drop,
        }
    }

    #[must_use]
    pub fn harness(&self) -> &Arc<Mutex<HookHarness>> {
        &self.harness
    }

    pub fn hook(&self, site: EffectSiteId, phase: HookPhase) -> Injection {
        let injection = self
            .harness
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .hook(site, phase);
        self.carried(injection)
    }

    #[must_use]
    pub fn carried(&self, injection: Injection) -> Injection {
        if injection == Injection::Kill {
            export::export(&self.harness);
        }
        injection
    }
}

impl Default for Exported {
    fn default() -> Self {
        Self::new(Arc::default())
    }
}

#[derive(Debug)]
struct ExportOnDrop(Arc<Mutex<HookHarness>>);

impl Drop for ExportOnDrop {
    fn drop(&mut self) {
        export::export(&self.0);
    }
}

#[cfg(test)]
mod export {
    use std::path::Path;
    use std::sync::{Arc, Mutex, PoisonError};

    use super::{OBSERVATIONS_ENV, ObservationRecord};
    use crate::topology::effects::HookHarness;

    pub(super) fn export(harness: &Arc<Mutex<HookHarness>>) {
        let Ok(dir) = std::env::var(OBSERVATIONS_ENV) else {
            return;
        };
        export_into(Path::new(&dir), harness);
    }

    pub(crate) fn export_into(dir: &Path, harness: &Arc<Mutex<HookHarness>>) {
        let thread = std::thread::current();
        let test = thread.name().unwrap_or("unnamed");
        let mut record = {
            let harness = harness.lock().unwrap_or_else(PoisonError::into_inner);
            ObservationRecord::of(test, &harness)
        };
        if record.is_empty() {
            return;
        }
        let name = format!("{test}.{}", std::process::id());
        let path = dir.join(ObservationRecord::file_name(&name));
        crate::workspace_manager::fixture::update_file(&path, |held| {
            if let Some(bytes) = held {
                if let Ok(earlier) = serde_json::from_slice::<ObservationRecord>(bytes) {
                    record.merge(earlier);
                }
            }
            serde_json::to_vec_pretty(&record).ok()
        });
    }
}

#[cfg(not(test))]
mod export {
    use std::sync::{Arc, Mutex};

    use crate::topology::effects::HookHarness;

    pub(super) fn export(_harness: &Arc<Mutex<HookHarness>>) {}
}

#[cfg(test)]
pub(crate) use export::export_into;
