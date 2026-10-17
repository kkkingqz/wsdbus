//! Shared pieces of providers written in Rust: the error reply of a rejected
//! command and the sandbox check of the caller (W20).

use wsdbus_contract::error::Rejection;
use wsdbus_contract::sandbox::{self, Evidence, Probe, Verdict};
use zbus::message::{Header, Message};
use zbus::names::ErrorName;
use zbus::{Connection, DBusError};

/// A rejected command as a D-Bus error reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderError(pub Rejection);

impl From<Rejection> for ProviderError {
    fn from(rejection: Rejection) -> Self {
        Self(rejection)
    }
}

impl DBusError for ProviderError {
    fn create_reply(&self, call: &Header<'_>) -> zbus::Result<Message> {
        Message::error(call, self.name())?.build(&(self.0.message.as_str(),))
    }

    fn name(&self) -> ErrorName<'_> {
        ErrorName::from_static_str_unchecked(self.0.kind.dbus_name())
    }

    fn description(&self) -> Option<&str> {
        Some(&self.0.message)
    }
}

/// Classifies the sender of a method call by its PID and `/proc`.
pub async fn caller_verdict(conn: &Connection, header: &Header<'_>) -> Verdict {
    let pid = match header.sender() {
        Some(sender) => match zbus::fdo::DBusProxy::new(conn).await {
            Ok(bus) => bus
                .get_connection_unix_process_id(sender.clone().into())
                .await
                .ok(),
            Err(_) => None,
        },
        None => None,
    };
    let evidence = match pid {
        Some(pid) => sandbox::probe_pid(pid),
        None => Evidence {
            pid: false,
            flatpak_info: Probe::Failed,
            cgroup: Probe::Failed,
        },
    };
    sandbox::classify(&evidence)
}
