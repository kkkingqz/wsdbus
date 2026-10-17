//! Desktop facts: keys, value types and validation.

use std::collections::{BTreeMap, BTreeSet};

/// A known fact. Unknown fact names on the wire are ignored by consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FactKey {
    FocusApp,
    FocusClass,
    FocusFullscreen,
    SessionLocked,
    AppearanceScheme,
    AppearanceAccent,
    AppearanceFont,
    Workspaces,
    WorkspaceActive,
    DisplayBrightness,
    KeyboardLayout,
}

/// The value type a fact carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Text,
    Bool,
    Number,
    Workspaces,
}

impl FactKey {
    pub const ALL: [FactKey; 11] = [
        FactKey::FocusApp,
        FactKey::FocusClass,
        FactKey::FocusFullscreen,
        FactKey::SessionLocked,
        FactKey::AppearanceScheme,
        FactKey::AppearanceAccent,
        FactKey::AppearanceFont,
        FactKey::Workspaces,
        FactKey::WorkspaceActive,
        FactKey::DisplayBrightness,
        FactKey::KeyboardLayout,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FactKey::FocusApp => "focus.app",
            FactKey::FocusClass => "focus.class",
            FactKey::FocusFullscreen => "focus.fullscreen",
            FactKey::SessionLocked => "session.locked",
            FactKey::AppearanceScheme => "appearance.scheme",
            FactKey::AppearanceAccent => "appearance.accent",
            FactKey::AppearanceFont => "appearance.font",
            FactKey::Workspaces => "workspaces",
            FactKey::WorkspaceActive => "workspace.active",
            FactKey::DisplayBrightness => "display.brightness",
            FactKey::KeyboardLayout => "keyboard.layout",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == name)
    }

    pub fn kind(self) -> ValueKind {
        match self {
            FactKey::FocusFullscreen | FactKey::SessionLocked => ValueKind::Bool,
            FactKey::DisplayBrightness => ValueKind::Number,
            FactKey::Workspaces => ValueKind::Workspaces,
            FactKey::FocusApp
            | FactKey::FocusClass
            | FactKey::AppearanceScheme
            | FactKey::AppearanceAccent
            | FactKey::AppearanceFont
            | FactKey::WorkspaceActive
            | FactKey::KeyboardLayout => ValueKind::Text,
        }
    }
}

impl std::fmt::Display for FactKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FactValue {
    Text(String),
    Bool(bool),
    Number(f64),
    Workspaces(Vec<Workspace>),
}

impl FactValue {
    pub fn kind(&self) -> ValueKind {
        match self {
            FactValue::Text(_) => ValueKind::Text,
            FactValue::Bool(_) => ValueKind::Bool,
            FactValue::Number(_) => ValueKind::Number,
            FactValue::Workspaces(_) => ValueKind::Workspaces,
        }
    }
}

/// Floating mode of a workspace. Only a Hyprland provider with `policy`
/// reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceMode {
    Tiling,
    Floating,
}

impl WorkspaceMode {
    pub fn name(self) -> &'static str {
        match self {
            WorkspaceMode::Tiling => "tiling",
            WorkspaceMode::Floating => "floating",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "tiling" => Some(WorkspaceMode::Tiling),
            "floating" => Some(WorkspaceMode::Floating),
            _ => None,
        }
    }
}

/// One record of the `workspaces` fact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub monitor: String,
    pub active: bool,
    pub mode: Option<WorkspaceMode>,
}

pub type Facts = BTreeMap<FactKey, FactValue>;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid fact {key}: {reason}")]
pub struct InvalidFact {
    pub key: String,
    pub reason: String,
}

impl InvalidFact {
    pub(crate) fn new(key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            reason: reason.into(),
        }
    }
}

/// Checks the value type and the value rules of a fact.
pub fn validate(key: FactKey, value: &FactValue) -> Result<(), InvalidFact> {
    let fail = |reason: &str| Err(InvalidFact::new(key.name(), reason));
    if value.kind() != key.kind() {
        return fail("wrong value type");
    }
    match (key, value) {
        (FactKey::AppearanceScheme, FactValue::Text(text)) if text != "light" && text != "dark" => {
            fail("expected light or dark")
        }
        (FactKey::AppearanceAccent, FactValue::Text(text)) if !is_hex_color(text) => {
            fail("expected #rrggbb in lower case")
        }
        (FactKey::DisplayBrightness, FactValue::Number(level))
            if !(level.is_finite() && (0.0..=1.0).contains(level)) =>
        {
            fail("expected a number from 0.0 to 1.0")
        }
        (FactKey::Workspaces, FactValue::Workspaces(records)) => {
            let mut ids = BTreeSet::new();
            for record in records {
                if record.id.is_empty() {
                    return fail("empty workspace id");
                }
                if !ids.insert(record.id.as_str()) {
                    return fail("duplicate workspace id");
                }
            }
            Ok(())
        }
        (_, FactValue::Text(text)) if text.is_empty() => fail("empty text"),
        _ => Ok(()),
    }
}

fn is_hex_color(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for key in FactKey::ALL {
            assert_eq!(FactKey::from_name(key.name()), Some(key));
        }
        assert_eq!(FactKey::from_name("app.id"), None);
    }

    #[test]
    fn value_rules() {
        let text = |text: &str| FactValue::Text(text.into());
        assert!(validate(FactKey::AppearanceScheme, &text("dark")).is_ok());
        assert!(validate(FactKey::AppearanceScheme, &text("default")).is_err());
        assert!(validate(FactKey::AppearanceAccent, &text("#3584e4")).is_ok());
        assert!(validate(FactKey::AppearanceAccent, &text("#3584E4")).is_err());
        assert!(validate(FactKey::AppearanceAccent, &text("blue")).is_err());
        assert!(validate(FactKey::DisplayBrightness, &FactValue::Number(0.5)).is_ok());
        assert!(validate(FactKey::DisplayBrightness, &FactValue::Number(1.5)).is_err());
        assert!(validate(FactKey::DisplayBrightness, &FactValue::Number(f64::NAN)).is_err());
        assert!(validate(FactKey::FocusApp, &text("")).is_err());
        assert!(validate(FactKey::FocusApp, &FactValue::Bool(true)).is_err());
        let workspace = |id: &str| Workspace {
            id: id.into(),
            name: id.into(),
            monitor: String::new(),
            active: false,
            mode: None,
        };
        let records =
            |ids: &[&str]| FactValue::Workspaces(ids.iter().map(|id| workspace(id)).collect());
        assert!(validate(FactKey::Workspaces, &records(&["1", "2"])).is_ok());
        assert!(validate(FactKey::Workspaces, &records(&["1", "1"])).is_err());
        assert!(validate(FactKey::Workspaces, &records(&[""])).is_err());
    }
}
