//! The wsdbus `Desktop1` contract: names, fact and window types, commands,
//! errors, the `app.id` and sandbox rules, the D-Bus and JSON encodings, the
//! provider-side store and the consumer-side mirror.
//!
//! Nothing here touches a bus. The normative description is `spec/` in the
//! repository; the cases in `spec/*.json` are tested against this crate.

pub mod admission;
pub mod appid;
pub mod command;
pub mod error;
pub mod fact;
pub mod json;
pub mod mirror;
pub mod sandbox;
pub mod store;
pub mod window;
pub mod wire;

pub use admission::admit;
pub use command::{Command, WindowAction};
pub use error::ErrorKind;
pub use fact::{FactKey, FactValue, Facts, InvalidFact, ValueKind, Workspace, WorkspaceMode};
pub use mirror::{Applied, Mirror};
pub use store::{Change, Store};
pub use window::{InvalidWindow, Window};

/// Well-known bus name of the provider.
pub const BUS_NAME: &str = "io.github.kkkingqz.WsDbus";
/// Object path of the provider.
pub const OBJECT_PATH: &str = "/io/github/kkkingqz/WsDbus";
/// Interface name of contract version 1.
pub const INTERFACE: &str = "io.github.kkkingqz.WsDbus.Desktop1";
/// Value of the `Version` property for this interface.
pub const VERSION: u32 = 1;

/// Capability announcing `GetWindows` and `WindowsChanged`.
pub const CAPABILITY_WINDOWS: &str = "windows";

/// Every capability a provider may announce: fact names, `windows` and
/// command method names.
pub fn all_capabilities() -> Vec<String> {
    FactKey::ALL
        .iter()
        .map(|key| key.name())
        .chain([CAPABILITY_WINDOWS])
        .chain(Command::METHODS)
        .map(str::to_owned)
        .collect()
}
