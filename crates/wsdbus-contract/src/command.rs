//! Commands, one per mutating method of `Desktop1`.

use crate::fact::WorkspaceMode;

/// Maximum number of keys in one `Shortcut`.
pub const MAX_SHORTCUT_KEYS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowAction {
    Activate,
    Close,
    Minimize,
    Restore,
    ToggleMaximize,
}

impl WindowAction {
    pub const ALL: [WindowAction; 5] = [
        WindowAction::Activate,
        WindowAction::Close,
        WindowAction::Minimize,
        WindowAction::Restore,
        WindowAction::ToggleMaximize,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WindowAction::Activate => "activate",
            WindowAction::Close => "close",
            WindowAction::Minimize => "minimize",
            WindowAction::Restore => "restore",
            WindowAction::ToggleMaximize => "toggle-maximize",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.name() == name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Press `keys` in order and release them in reverse order. With `app`,
    /// only while that app is in focus; otherwise nothing happens.
    Shortcut {
        keys: Vec<String>,
        app: Option<String>,
    },
    /// Launch an app by its `app.id`.
    LaunchApp {
        id: String,
    },
    ActivateWorkspace {
        id: String,
    },
    WindowAction {
        id: String,
        action: WindowAction,
    },
    SetWorkspaceMode {
        id: String,
        mode: WorkspaceMode,
    },
    /// Press or release one key, for surfaces without their own key input.
    /// The provider releases keys a caller still holds when the caller
    /// leaves the bus or the screen locks.
    Key {
        key: String,
        pressed: bool,
    },
    /// Set the screen brightness, 0.0 to 1.0.
    SetBrightness {
        level: f64,
    },
}

impl Command {
    /// Method names, which are also the command capability names.
    pub const METHODS: [&'static str; 7] = [
        "Shortcut",
        "LaunchApp",
        "ActivateWorkspace",
        "WindowAction",
        "SetWorkspaceMode",
        "Key",
        "SetBrightness",
    ];

    pub fn method(&self) -> &'static str {
        match self {
            Command::Shortcut { .. } => "Shortcut",
            Command::LaunchApp { .. } => "LaunchApp",
            Command::ActivateWorkspace { .. } => "ActivateWorkspace",
            Command::WindowAction { .. } => "WindowAction",
            Command::SetWorkspaceMode { .. } => "SetWorkspaceMode",
            Command::Key { .. } => "Key",
            Command::SetBrightness { .. } => "SetBrightness",
        }
    }

    /// Commands that act on behalf of the user in an app; rejected with
    /// `Locked` while the screen is locked. Releasing a key is always
    /// allowed.
    pub fn blocked_when_locked(&self) -> bool {
        matches!(
            self,
            Command::Shortcut { .. }
                | Command::LaunchApp { .. }
                | Command::Key { pressed: true, .. }
        )
    }

    /// Checks the arguments; failure is `InvalidArgs`.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Command::Shortcut { keys, app } => {
                if keys.is_empty() || keys.len() > MAX_SHORTCUT_KEYS {
                    return Err(format!("expected 1 to {MAX_SHORTCUT_KEYS} keys"));
                }
                if keys
                    .iter()
                    .any(|key| key.is_empty() || key.contains(char::is_whitespace))
                {
                    return Err("empty key name or key name with spaces".into());
                }
                if app.as_deref() == Some("") {
                    return Err("empty app; use no app instead".into());
                }
                Ok(())
            }
            Command::Key { key, .. } => {
                if key.is_empty() || key.contains(char::is_whitespace) {
                    Err("empty key name or key name with spaces".into())
                } else {
                    Ok(())
                }
            }
            Command::SetBrightness { level } => {
                if level.is_finite() && (0.0..=1.0).contains(level) {
                    Ok(())
                } else {
                    Err("expected a level from 0.0 to 1.0".into())
                }
            }
            Command::LaunchApp { id }
            | Command::ActivateWorkspace { id }
            | Command::WindowAction { id, .. }
            | Command::SetWorkspaceMode { id, .. } => {
                if id.is_empty() {
                    Err("empty id".into())
                } else {
                    Ok(())
                }
            }
        }
    }
}
