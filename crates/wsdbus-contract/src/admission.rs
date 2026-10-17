//! The checks every provider runs before a command, in contract order.

use crate::command::Command;
use crate::error::{ErrorKind, Rejection};
use crate::sandbox::Verdict;

/// Checks a command before the provider executes it: arguments, then the
/// caller, then the capability, then the lock. Checks that need provider
/// state (window ids, workspace ids) come after this.
pub fn admit(
    command: &Command,
    caller: &Verdict,
    capabilities: &[String],
    locked: bool,
) -> Result<(), Rejection> {
    command
        .validate()
        .map_err(|message| Rejection::new(ErrorKind::InvalidArgs, message))?;
    if *caller != Verdict::Trusted {
        return Err(Rejection::new(ErrorKind::Denied, caller.describe()));
    }
    let method = command.method();
    if !capabilities.iter().any(|capability| capability == method) {
        return Err(Rejection::new(
            ErrorKind::Unsupported,
            format!("{method} is not supported by this provider"),
        ));
    }
    if locked && command.blocked_when_locked() {
        return Err(Rejection::new(ErrorKind::Locked, "the screen is locked"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::Sandbox;

    fn launch() -> Command {
        Command::LaunchApp {
            id: "org.gnome.Nautilus".into(),
        }
    }

    fn kind(result: Result<(), Rejection>) -> Option<ErrorKind> {
        result.err().map(|rejection| rejection.kind)
    }

    #[test]
    fn order_of_checks() {
        let all = crate::all_capabilities();
        let sandboxed = Verdict::Sandboxed(Sandbox::Flatpak);
        let empty = Command::LaunchApp { id: String::new() };
        assert_eq!(
            kind(admit(&empty, &sandboxed, &[], true)),
            Some(ErrorKind::InvalidArgs)
        );
        assert_eq!(
            kind(admit(&launch(), &sandboxed, &[], true)),
            Some(ErrorKind::Denied)
        );
        assert_eq!(
            kind(admit(&launch(), &Verdict::Unverified, &all, false)),
            Some(ErrorKind::Denied)
        );
        assert_eq!(
            kind(admit(&launch(), &Verdict::Trusted, &[], true)),
            Some(ErrorKind::Unsupported)
        );
        assert_eq!(
            kind(admit(&launch(), &Verdict::Trusted, &all, true)),
            Some(ErrorKind::Locked)
        );
        assert_eq!(kind(admit(&launch(), &Verdict::Trusted, &all, false)), None);
        let close = Command::WindowAction {
            id: "1".into(),
            action: crate::WindowAction::Close,
        };
        assert_eq!(kind(admit(&close, &Verdict::Trusted, &all, true)), None);
    }
}
