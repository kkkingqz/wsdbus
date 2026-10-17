//! Errors of the mutating methods.

/// Prefix of the contract's own D-Bus error names.
pub const ERROR_PREFIX: &str = "io.github.kkkingqz.WsDbus.Error";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The provider does not support this command, action or mode.
    Unsupported,
    /// No window with this id exists in the session.
    StaleWindow,
    /// The screen is locked.
    Locked,
    /// The caller is sandboxed or could not be checked.
    Denied,
    /// Malformed arguments, an unknown workspace or app.
    InvalidArgs,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 5] = [
        ErrorKind::Unsupported,
        ErrorKind::StaleWindow,
        ErrorKind::Locked,
        ErrorKind::Denied,
        ErrorKind::InvalidArgs,
    ];

    pub fn dbus_name(self) -> &'static str {
        match self {
            ErrorKind::Unsupported => "io.github.kkkingqz.WsDbus.Error.Unsupported",
            ErrorKind::StaleWindow => "io.github.kkkingqz.WsDbus.Error.StaleWindow",
            ErrorKind::Locked => "io.github.kkkingqz.WsDbus.Error.Locked",
            ErrorKind::Denied => "io.github.kkkingqz.WsDbus.Error.Denied",
            ErrorKind::InvalidArgs => "org.freedesktop.DBus.Error.InvalidArgs",
        }
    }

    pub fn from_dbus_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.dbus_name() == name)
    }

    /// Short name used by `wsdbus-ctl` and JSON output.
    pub fn name(self) -> &'static str {
        match self {
            ErrorKind::Unsupported => "unsupported",
            ErrorKind::StaleWindow => "stale",
            ErrorKind::Locked => "locked",
            ErrorKind::Denied => "denied",
            ErrorKind::InvalidArgs => "invalid-args",
        }
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A rejected command: error kind and a human-readable message.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind}: {message}")]
pub struct Rejection {
    pub kind: ErrorKind,
    pub message: String,
}

impl Rejection {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
