//! zbus client of the wsdbus `Desktop1` contract.
//!
//! [`Client`] runs commands and one-off queries; [`Watcher`] keeps a copy of
//! the provider state across provider restarts. [`provider`] has the pieces
//! every provider written in Rust shares: the error reply and the caller
//! check.

pub mod provider;
mod watch;

use std::collections::BTreeMap;

use wsdbus_contract::wire::{self, Decoded, WireDict};
use wsdbus_contract::{BUS_NAME, Command, ErrorKind, Facts, INTERFACE, OBJECT_PATH, Window};
use zbus::Connection;
use zbus::zvariant::OwnedValue;

pub use watch::{Snapshot, Update, Watcher};

/// Provider properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub version: u32,
    pub provider: String,
    pub capabilities: Vec<String>,
}

impl Info {
    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|item| item == capability)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No provider owns the bus name.
    #[error("no wsdbus provider on the bus")]
    NoProvider,
    /// The provider rejected the command.
    #[error("{kind}: {message}")]
    Rejected { kind: ErrorKind, message: String },
    /// The provider answered with something outside the contract.
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error(transparent)]
    Bus(zbus::Error),
}

impl Error {
    pub fn kind(&self) -> Option<ErrorKind> {
        match self {
            Error::Rejected { kind, .. } => Some(*kind),
            _ => None,
        }
    }
}

const NO_PROVIDER_ERRORS: [&str; 3] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
    "org.freedesktop.DBus.Error.NoReply",
];

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        if let zbus::Error::MethodError(name, description, _) = &error {
            if NO_PROVIDER_ERRORS.contains(&name.as_str()) {
                return Error::NoProvider;
            }
            if let Some(kind) = ErrorKind::from_dbus_name(name.as_str()) {
                return Error::Rejected {
                    kind,
                    message: description.clone().unwrap_or_default(),
                };
            }
        }
        Error::Bus(error)
    }
}

impl From<zbus::fdo::Error> for Error {
    fn from(error: zbus::fdo::Error) -> Self {
        Error::from(zbus::Error::from(error))
    }
}

fn property<T>(all: &BTreeMap<String, OwnedValue>, name: &str) -> Result<T, Error>
where
    T: TryFrom<OwnedValue>,
{
    let value = all
        .get(name)
        .ok_or_else(|| Error::Protocol(format!("missing property {name}")))?;
    value
        .try_clone()
        .ok()
        .and_then(|value| T::try_from(value).ok())
        .ok_or_else(|| Error::Protocol(format!("property {name} has a wrong type")))
}

/// Commands and one-off queries.
#[derive(Clone, Debug)]
pub struct Client {
    conn: Connection,
}

impl Client {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Connects to the session bus.
    pub async fn session() -> zbus::Result<Self> {
        Ok(Self::new(Connection::session().await?))
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    async fn call<B, R>(&self, method: &str, body: &B) -> Result<R, Error>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
    {
        let reply = self
            .conn
            .call_method(Some(BUS_NAME), OBJECT_PATH, Some(INTERFACE), method, body)
            .await?;
        reply
            .body()
            .deserialize::<R>()
            .map_err(|error| Error::Protocol(format!("{method} reply: {error}")))
    }

    pub async fn info(&self) -> Result<Info, Error> {
        let properties = zbus::fdo::PropertiesProxy::builder(&self.conn)
            .destination(BUS_NAME)?
            .path(OBJECT_PATH)?
            .build()
            .await?;
        let all: BTreeMap<String, OwnedValue> = properties
            .get_all(zbus::names::InterfaceName::from_static_str_unchecked(
                INTERFACE,
            ))
            .await?
            .into_iter()
            .collect();
        Ok(Info {
            version: property(&all, "Version")?,
            provider: property(&all, "Provider")?,
            capabilities: property(&all, "Capabilities")?,
        })
    }

    /// Facts snapshot.
    pub async fn state(&self) -> Result<(u64, Decoded<Facts>), Error> {
        let (revision, facts): (u64, WireDict) = self.call("GetState", &()).await?;
        Ok((revision, wire::decode_facts(&facts)))
    }

    /// Windows snapshot.
    pub async fn windows(&self) -> Result<(u64, Decoded<Vec<Window>>), Error> {
        let (revision, windows): (u64, Vec<WireDict>) = self.call("GetWindows", &()).await?;
        Ok((revision, wire::decode_windows(&windows)))
    }

    pub async fn run(&self, command: &Command) -> Result<(), Error> {
        let method = command.method();
        match command {
            Command::Shortcut { keys, app } => {
                self.call(method, &(keys, app.as_deref().unwrap_or_default()))
                    .await
            }
            Command::LaunchApp { id } | Command::ActivateWorkspace { id } => {
                self.call(method, &(id,)).await
            }
            Command::WindowAction { id, action } => self.call(method, &(id, action.name())).await,
            Command::SetWorkspaceMode { id, mode } => self.call(method, &(id, mode.name())).await,
            Command::Key { key, pressed } => self.call(method, &(key, *pressed)).await,
            Command::SetBrightness { level } => self.call(method, &(*level,)).await,
        }
    }

    /// Starts watching the provider state.
    pub async fn watch(&self) -> zbus::Result<Watcher> {
        Watcher::new(self.clone()).await
    }
}
