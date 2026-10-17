//! Consumer-side copy of the provider state.
//!
//! The client subscribes to the signals, then takes the facts snapshot and
//! the windows snapshot; each carries its own revision. A change applies to
//! its part when its revision is newer than that part. All changes share one
//! counter, so a revision more than one past the newest seen means a change
//! was missed and a new snapshot is needed.

use std::collections::BTreeMap;

use crate::fact::Facts;
use crate::store::Change;
use crate::window::Window;

#[derive(Clone, Debug, PartialEq)]
pub enum Applied {
    /// The change was applied.
    Changed,
    /// The change is already contained in a snapshot.
    Stale,
    /// A change was missed; take a new snapshot.
    Gap,
    /// No snapshot yet.
    NotSynced,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mirror {
    synced: bool,
    facts_revision: u64,
    windows_revision: u64,
    last: u64,
    facts: Facts,
    windows: BTreeMap<String, Window>,
}

impl Mirror {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_synced(&self) -> bool {
        self.synced
    }

    /// Newest revision applied.
    pub fn revision(&self) -> u64 {
        self.last
    }

    pub fn facts(&self) -> &Facts {
        &self.facts
    }

    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.windows.values()
    }

    pub fn snapshot(
        &mut self,
        facts_revision: u64,
        facts: Facts,
        windows_revision: u64,
        windows: Vec<Window>,
    ) {
        *self = Self {
            synced: true,
            facts_revision,
            windows_revision,
            last: facts_revision.max(windows_revision),
            facts,
            windows: windows
                .into_iter()
                .map(|window| (window.id.clone(), window))
                .collect(),
        };
    }

    /// Forgets the state, for a lost provider.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn apply(&mut self, change: &Change) -> Applied {
        if !self.synced {
            return Applied::NotSynced;
        }
        let revision = change.revision();
        let part = match change {
            Change::Facts { .. } => self.facts_revision,
            Change::Windows { .. } => self.windows_revision,
        };
        if revision <= part {
            return Applied::Stale;
        }
        if revision > self.last + 1 {
            self.synced = false;
            return Applied::Gap;
        }
        match change {
            Change::Facts {
                changed, removed, ..
            } => {
                for key in removed {
                    self.facts.remove(key);
                }
                self.facts
                    .extend(changed.iter().map(|(key, value)| (*key, value.clone())));
                self.facts_revision = revision;
            }
            Change::Windows {
                upserted, removed, ..
            } => {
                for id in removed {
                    self.windows.remove(id);
                }
                self.windows.extend(
                    upserted
                        .iter()
                        .map(|window| (window.id.clone(), window.clone())),
                );
                self.windows_revision = revision;
            }
        }
        self.last = self.last.max(revision);
        Applied::Changed
    }
}
