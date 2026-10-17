//! Sandbox check of a caller (W20): mutating methods are denied to sandboxed
//! processes and to processes that could not be checked.

/// Result of reading one piece of evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe<T> {
    Present(T),
    /// The file does not exist.
    Absent,
    /// The file could not be read for another reason.
    Failed,
}

/// Evidence about a caller, collected by the provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    /// Whether the bus returned the caller's PID.
    pub pid: bool,
    /// `/proc/PID/root/.flatpak-info`.
    pub flatpak_info: Probe<()>,
    /// Contents of `/proc/PID/cgroup`.
    pub cgroup: Probe<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sandbox {
    Flatpak,
    Snap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Trusted,
    Sandboxed(Sandbox),
    Unverified,
}

impl Verdict {
    pub fn name(&self) -> &'static str {
        match self {
            Verdict::Trusted => "trusted",
            Verdict::Sandboxed(_) => "sandboxed",
            Verdict::Unverified => "unverified",
        }
    }

    pub fn describe(&self) -> &'static str {
        match self {
            Verdict::Trusted => "the caller is trusted",
            Verdict::Sandboxed(Sandbox::Flatpak) => "the caller runs in a Flatpak sandbox",
            Verdict::Sandboxed(Sandbox::Snap) => "the caller runs in a Snap sandbox",
            Verdict::Unverified => "the caller could not be checked",
        }
    }
}

/// Classifies a caller by its evidence.
pub fn classify(evidence: &Evidence) -> Verdict {
    if !evidence.pid {
        return Verdict::Unverified;
    }
    match evidence.flatpak_info {
        Probe::Present(()) => return Verdict::Sandboxed(Sandbox::Flatpak),
        Probe::Failed => return Verdict::Unverified,
        Probe::Absent => {}
    }
    let Probe::Present(cgroup) = &evidence.cgroup else {
        return Verdict::Unverified;
    };
    for line in cgroup.lines() {
        // cgroup v1 lines are `ID:CONTROLLERS:PATH`, v2 lines are `0::PATH`.
        let Some(path) = line.splitn(3, ':').nth(2) else {
            continue;
        };
        for unit in path.split('/') {
            if unit.starts_with("app-flatpak-") {
                return Verdict::Sandboxed(Sandbox::Flatpak);
            }
            if unit.starts_with("snap.") {
                return Verdict::Sandboxed(Sandbox::Snap);
            }
        }
    }
    Verdict::Trusted
}

/// Reads the evidence for a PID from `/proc`.
#[cfg(feature = "proc")]
pub fn probe_pid(pid: u32) -> Evidence {
    use std::io::ErrorKind;
    let flatpak_info = match std::fs::symlink_metadata(format!("/proc/{pid}/root/.flatpak-info")) {
        Ok(_) => Probe::Present(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Probe::Absent,
        Err(_) => Probe::Failed,
    };
    let cgroup = match std::fs::read_to_string(format!("/proc/{pid}/cgroup")) {
        Ok(text) => Probe::Present(text),
        Err(error) if error.kind() == ErrorKind::NotFound => Probe::Absent,
        Err(_) => Probe::Failed,
    };
    Evidence {
        pid: true,
        flatpak_info,
        cgroup,
    }
}
