//! Provider-side state with one revision counter for facts and windows.

use std::collections::BTreeMap;

use crate::fact::{self, FactKey, Facts, InvalidFact};
use crate::window::{InvalidWindow, Window};

/// One state change, the payload of `StateChanged` or `WindowsChanged`.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Facts {
        revision: u64,
        changed: Facts,
        removed: Vec<FactKey>,
    },
    Windows {
        revision: u64,
        upserted: Vec<Window>,
        removed: Vec<String>,
    },
}

impl Change {
    pub fn revision(&self) -> u64 {
        match self {
            Change::Facts { revision, .. } | Change::Windows { revision, .. } => *revision,
        }
    }
}

/// Facts and windows of a provider. Every effective update bumps the
/// revision by one and yields exactly one [`Change`] to emit; updates that
/// change nothing yield nothing and keep the revision.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Store {
    revision: u64,
    facts: Facts,
    windows: BTreeMap<String, Window>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn facts(&self) -> &Facts {
        &self.facts
    }

    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.windows.values()
    }

    pub fn window(&self, id: &str) -> Option<&Window> {
        self.windows.get(id)
    }

    /// Sets `changed` and removes `removed`. A key in both is set.
    pub fn update_facts(
        &mut self,
        changed: Facts,
        removed: &[FactKey],
    ) -> Result<Option<Change>, InvalidFact> {
        for (key, value) in &changed {
            fact::validate(*key, value)?;
        }
        let changed: Facts = changed
            .into_iter()
            .filter(|(key, value)| self.facts.get(key) != Some(value))
            .collect();
        let mut removed: Vec<FactKey> = removed
            .iter()
            .copied()
            .filter(|key| self.facts.contains_key(key) && !changed.contains_key(key))
            .collect();
        removed.sort();
        removed.dedup();
        if changed.is_empty() && removed.is_empty() {
            return Ok(None);
        }
        for key in &removed {
            self.facts.remove(key);
        }
        self.facts
            .extend(changed.iter().map(|(key, value)| (*key, value.clone())));
        self.revision += 1;
        Ok(Some(Change::Facts {
            revision: self.revision,
            changed,
            removed,
        }))
    }

    /// Inserts or replaces `upserted` and removes `removed`. An id in both is
    /// upserted.
    pub fn update_windows(
        &mut self,
        upserted: Vec<Window>,
        removed: &[String],
    ) -> Result<Option<Change>, InvalidWindow> {
        for window in &upserted {
            window.validate()?;
        }
        let mut changed: BTreeMap<String, Window> = BTreeMap::new();
        for window in upserted {
            changed.insert(window.id.clone(), window);
        }
        changed.retain(|id, window| self.windows.get(id) != Some(window));
        let mut removed: Vec<String> = removed
            .iter()
            .filter(|id| self.windows.contains_key(*id) && !changed.contains_key(*id))
            .cloned()
            .collect();
        removed.sort();
        removed.dedup();
        if changed.is_empty() && removed.is_empty() {
            return Ok(None);
        }
        for id in &removed {
            self.windows.remove(id);
        }
        self.windows.extend(changed.clone());
        self.revision += 1;
        Ok(Some(Change::Windows {
            revision: self.revision,
            upserted: changed.into_values().collect(),
            removed,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact::FactValue;

    fn window(id: &str, title: &str) -> Window {
        Window {
            id: id.into(),
            app: "app".into(),
            title: title.into(),
            ..Window::default()
        }
    }

    #[test]
    fn no_op_updates_keep_revision() {
        let mut store = Store::new();
        let locked = Facts::from([(FactKey::SessionLocked, FactValue::Bool(false))]);
        assert_eq!(
            store
                .update_facts(locked.clone(), &[])
                .unwrap()
                .unwrap()
                .revision(),
            1
        );
        assert_eq!(
            store.update_facts(locked, &[FactKey::FocusApp]).unwrap(),
            None
        );
        assert_eq!(
            store
                .update_windows(vec![window("1", "a")], &[])
                .unwrap()
                .unwrap()
                .revision(),
            2
        );
        assert_eq!(
            store
                .update_windows(vec![window("1", "a")], &["2".into()])
                .unwrap(),
            None
        );
        assert_eq!(store.revision(), 2);
    }

    #[test]
    fn invalid_updates_change_nothing() {
        let mut store = Store::new();
        let bad = Facts::from([(FactKey::AppearanceScheme, FactValue::Text("sepia".into()))]);
        assert!(store.update_facts(bad, &[]).is_err());
        assert!(store.update_windows(vec![window("", "a")], &[]).is_err());
        assert_eq!(store, Store::new());
    }

    #[test]
    fn upsert_wins_over_remove() {
        let mut store = Store::new();
        store.update_windows(vec![window("1", "a")], &[]).unwrap();
        let change = store
            .update_windows(vec![window("1", "b")], &["1".into()])
            .unwrap()
            .unwrap();
        assert_eq!(
            change,
            Change::Windows {
                revision: 2,
                upserted: vec![window("1", "b")],
                removed: vec![],
            }
        );
    }
}
