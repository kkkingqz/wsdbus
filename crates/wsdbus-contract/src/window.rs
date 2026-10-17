//! Window records of `GetWindows` and `WindowsChanged`.

/// One window. `id` is opaque and valid within the session only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Window {
    pub id: String,
    pub app: String,
    pub class: String,
    pub title: String,
    pub workspace: String,
    pub monitor: String,
    pub focused: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub floating: bool,
    pub fullscreen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid window {id:?}: {reason}")]
pub struct InvalidWindow {
    pub id: String,
    pub reason: &'static str,
}

impl Window {
    pub fn validate(&self) -> Result<(), InvalidWindow> {
        let fail = |reason| {
            Err(InvalidWindow {
                id: self.id.clone(),
                reason,
            })
        };
        if self.id.is_empty() {
            return fail("empty id");
        }
        if self.app.is_empty() {
            return fail("empty app");
        }
        Ok(())
    }
}
