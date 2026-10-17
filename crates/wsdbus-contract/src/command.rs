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

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

impl Command {
    /// Method names, which are also the command capability names.
    pub const METHODS: [&'static str; 5] = [
        "Shortcut",
        "LaunchApp",
        "ActivateWorkspace",
        "WindowAction",
        "SetWorkspaceMode",
    ];

    pub fn method(&self) -> &'static str {
        match self {
            Command::Shortcut { .. } => "Shortcut",
            Command::LaunchApp { .. } => "LaunchApp",
            Command::ActivateWorkspace { .. } => "ActivateWorkspace",
            Command::WindowAction { .. } => "WindowAction",
            Command::SetWorkspaceMode { .. } => "SetWorkspaceMode",
        }
    }

    /// Commands that act on behalf of the user in an app; rejected with
    /// `Locked` while the screen is locked.
    pub fn blocked_when_locked(&self) -> bool {
        matches!(self, Command::Shortcut { .. } | Command::LaunchApp { .. })
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
