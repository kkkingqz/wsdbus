//! The `app.id` rule (W6).
//!
//! The provider finds the desktop file of a window (GNOME through
//! `Shell.WindowTracker`, Hyprland by its own lookup) and passes its id and
//! the window class here. The cases in `spec/appid-cases.json` must hold for
//! every provider.

/// Prefix of ids of windows without a desktop file.
pub const WMCLASS_PREFIX: &str = "wmclass:";

/// Normalized app id: the desktop file id without `.desktop`, otherwise
/// `wmclass:<class>`.
///
/// `desktop_id` values starting with `window:` are GNOME's ids for windows
/// without an app and count as no desktop file.
pub fn normalize(desktop_id: Option<&str>, class: Option<&str>) -> String {
    let desktop = desktop_id
        .map(str::trim)
        .map(|id| id.strip_suffix(".desktop").unwrap_or(id))
        .filter(|id| !id.is_empty() && !id.starts_with("window:"));
    match desktop {
        Some(id) => id.to_owned(),
        None => format!(
            "{WMCLASS_PREFIX}{}",
            class.map(str::trim).unwrap_or_default()
        ),
    }
}
