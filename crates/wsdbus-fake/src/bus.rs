//! A private `dbus-daemon` for tests, so they never touch the session bus.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use zbus::Connection;

/// A session-type `dbus-daemon` that lives as long as this value.
pub struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    /// Starts `dbus-daemon` from `PATH` with its own configuration: a
    /// socket in the temporary directory and no activatable services, so it
    /// needs no `/etc/dbus-1` (a Nix build has none).
    pub fn start() -> std::io::Result<Self> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir();
        let config = dir.join(format!(
            "wsdbus-test-bus-{}-{}.conf",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&config, bus_config(&dir.to_string_lossy()))?;
        let spawned = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                let _ = std::fs::remove_file(&config);
                return Err(error);
            }
        };
        let mut address = String::new();
        let stdout = child.stdout.take().expect("piped stdout");
        let read = BufReader::new(stdout).read_line(&mut address);
        // The daemon has read its configuration once it printed an address.
        let _ = std::fs::remove_file(&config);
        read?;
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

fn bus_config(dir: &str) -> String {
    let dir = dir
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir={dir}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#
    )
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
