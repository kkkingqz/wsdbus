//! Watching the provider: snapshot, changes, gaps and reconnects.

use std::time::{Duration, Instant};

use futures_lite::{StreamExt, future};
use wsdbus_contract::wire::{self, WireDict};
use wsdbus_contract::{Applied, BUS_NAME, Change, Facts, INTERFACE, Mirror, OBJECT_PATH, Window};
use zbus::fdo::{DBusProxy, NameOwnerChangedStream};
use zbus::message::Type as MessageType;
use zbus::names::OwnedUniqueName;
use zbus::{MatchRule, Message, MessageStream};

use crate::{Client, Error, Info};

const FIRST_RETRY: Duration = Duration::from_millis(100);
const LAST_RETRY: Duration = Duration::from_secs(5);
const QUEUED_SIGNALS: usize = 4096;

/// The full provider state at one point.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub info: Info,
    /// Newest revision contained in the snapshot.
    pub revision: u64,
    pub facts: Facts,
    pub windows: Vec<Window>,
    /// Fact names and window ids dropped as invalid.
    pub rejected: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    /// A provider appeared or the state was taken again after a gap: it
    /// replaces everything known before.
    Snapshot(Snapshot),
    /// A change on top of the last snapshot.
    Change {
        change: Change,
        rejected: Vec<String>,
    },
    /// The provider is gone; everything known before is void.
    Lost,
}

enum Event {
    Owner(Option<zbus::fdo::NameOwnerChanged>),
    Signal(Option<zbus::Result<Message>>),
    Retry,
}

/// Keeps a copy of the provider state; call [`Watcher::next`] in a loop.
pub struct Watcher {
    client: Client,
    owners: NameOwnerChangedStream,
    signals: MessageStream,
    owner: Option<OwnedUniqueName>,
    mirror: Mirror,
    need_snapshot: bool,
    backoff: Duration,
    retry_at: Option<Instant>,
}

impl Watcher {
    pub(crate) async fn new(client: Client) -> zbus::Result<Self> {
        let conn = client.connection().clone();
        let bus = DBusProxy::new(&conn).await?;
        let owners = bus
            .receive_name_owner_changed_with_args(&[(0, BUS_NAME)])
            .await?;
        let rule = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .interface(INTERFACE)?
            .path(OBJECT_PATH)?
            .build();
        let signals = MessageStream::for_match_rule(rule, &conn, Some(QUEUED_SIGNALS)).await?;
        let owner = match bus.get_name_owner(BUS_NAME.try_into()?).await {
            Ok(owner) => Some(owner),
            Err(zbus::fdo::Error::NameHasNoOwner(_)) => None,
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            client,
            owners,
            signals,
            owner,
            mirror: Mirror::new(),
            need_snapshot: true,
            backoff: FIRST_RETRY,
            retry_at: None,
        })
    }

    /// The state as of the last returned update.
    pub fn mirror(&self) -> &Mirror {
        &self.mirror
    }

    /// Unique name of the current provider.
    pub fn owner(&self) -> Option<&OwnedUniqueName> {
        self.owner.as_ref()
    }

    /// Waits for the next update. An error means the bus connection itself
    /// failed.
    pub async fn next(&mut self) -> zbus::Result<Update> {
        loop {
            let due = self.retry_at.is_none_or(|at| at <= Instant::now());
            if self.owner.is_some() && self.need_snapshot && due {
                match self.snapshot().await {
                    Ok(snapshot) => {
                        self.need_snapshot = false;
                        self.backoff = FIRST_RETRY;
                        self.retry_at = None;
                        return Ok(Update::Snapshot(snapshot));
                    }
                    Err(Error::Bus(error)) if is_disconnect(&error) => return Err(error),
                    Err(_) => {
                        self.retry_at = Some(Instant::now() + self.backoff);
                        self.backoff = (self.backoff * 2).min(LAST_RETRY);
                        if let Some(lost) = self.lose() {
                            return Ok(lost);
                        }
                    }
                }
            }
            if let Some(update) = self.wait().await? {
                return Ok(update);
            }
        }
    }

    fn lose(&mut self) -> Option<Update> {
        let synced = self.mirror.is_synced();
        self.mirror.reset();
        self.need_snapshot = true;
        synced.then_some(Update::Lost)
    }

    async fn wait(&mut self) -> zbus::Result<Option<Update>> {
        let retry_at = self
            .retry_at
            .filter(|_| self.owner.is_some() && self.need_snapshot);
        let owners = async { Event::Owner(self.owners.next().await) };
        let signals = async { Event::Signal(self.signals.next().await) };
        let retry = async {
            match retry_at {
                Some(at) => {
                    async_io::Timer::at(at).await;
                    Event::Retry
                }
                None => future::pending().await,
            }
        };
        match future::or(owners, future::or(signals, retry)).await {
            Event::Owner(None) | Event::Signal(None) => Err(zbus::Error::InputOutput(
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "bus connection closed").into(),
            )),
            Event::Owner(Some(signal)) => {
                let args = signal.args()?;
                self.owner = args
                    .new_owner()
                    .as_ref()
                    .map(|owner| owner.to_owned().into());
                self.retry_at = None;
                self.backoff = FIRST_RETRY;
                if self.owner.is_none() {
                    return Ok(self.lose());
                }
                // A new owner replaces the state with its snapshot.
                self.need_snapshot = true;
                Ok(None)
            }
            Event::Signal(Some(message)) => Ok(self.signal(&message?)),
            Event::Retry => Ok(None),
        }
    }

    fn signal(&mut self, message: &Message) -> Option<Update> {
        let header = message.header();
        let from_owner = match (header.sender(), &self.owner) {
            (Some(sender), Some(owner)) => sender == owner.inner(),
            _ => false,
        };
        if !from_owner || self.need_snapshot {
            return None;
        }
        let body = message.body();
        let (change, rejected) = match header.member().map(|member| member.as_str()) {
            Some("StateChanged") => {
                let (revision, changed, removed) =
                    body.deserialize::<(u64, WireDict, Vec<String>)>().ok()?;
                let decoded = wire::decode_facts(&changed);
                let change = Change::Facts {
                    revision,
                    changed: decoded.value,
                    removed: wire::decode_fact_names(&removed),
                };
                (change, decoded.rejected)
            }
            Some("WindowsChanged") => {
                let (revision, upserted, removed) = body
                    .deserialize::<(u64, Vec<WireDict>, Vec<String>)>()
                    .ok()?;
                let decoded = wire::decode_windows(&upserted);
                let change = Change::Windows {
                    revision,
                    upserted: decoded.value,
                    removed,
                };
                (change, decoded.rejected)
            }
            _ => return None,
        };
        match self.mirror.apply(&change) {
            Applied::Changed => Some(Update::Change { change, rejected }),
            Applied::Gap => {
                self.need_snapshot = true;
                None
            }
            Applied::Stale | Applied::NotSynced => None,
        }
    }

    async fn snapshot(&mut self) -> Result<Snapshot, Error> {
        let info = self.client.info().await?;
        let (facts_revision, facts) = self.client.state().await?;
        let (windows_revision, windows) = if info.supports(wsdbus_contract::CAPABILITY_WINDOWS) {
            self.client.windows().await?
        } else {
            (
                facts_revision,
                wire::Decoded {
                    value: Vec::new(),
                    rejected: Vec::new(),
                },
            )
        };
        let mut rejected = facts.rejected;
        rejected.extend(windows.rejected);
        self.mirror.snapshot(
            facts_revision,
            facts.value.clone(),
            windows_revision,
            windows.value.clone(),
        );
        Ok(Snapshot {
            info,
            revision: self.mirror.revision(),
            facts: facts.value,
            windows: self.mirror.windows().cloned().collect(),
            rejected,
        })
    }
}

fn is_disconnect(error: &zbus::Error) -> bool {
    matches!(error, zbus::Error::InputOutput(_))
}
