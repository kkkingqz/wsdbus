//! A private `dbus-daemon` for tests, so they never touch the session bus.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use zbus::Connection;

/// A session-type `dbus-daemon` that lives as long as this value.
pub struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    /// Starts `dbus-daemon` from `PATH`.
    pub fn start() -> std::io::Result<Self> {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut address = String::new();
        let stdout = child.stdout.take().expect("piped stdout");
        BufReader::new(stdout).read_line(&mut address)?;
        let address = address.trim().to_owned();
        if address.is_empty() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::other("dbus-daemon printed no address"));
        }
        Ok(Self { child, address })
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    /// A new connection to this bus.
    pub async fn connect(&self) -> zbus::Result<Connection> {
        zbus::connection::Builder::address(self.address.as_str())?
            .build()
            .await
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
