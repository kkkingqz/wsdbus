//! A fake wsdbus provider for consumer tests and development.
//!
//! [`Fake`] serves `Desktop1` on a connection with state set by the test.
//! Commands run the contract checks, are recorded, and window actions and
//! workspace commands change the state as a real provider would. Nothing
//! reaches a desktop. [`bus::PrivateBus`] starts a bus to serve it on.

pub mod bus;

use wsdbus_client::provider::{ProviderError, caller_verdict};
use wsdbus_contract::error::Rejection;
use wsdbus_contract::sandbox::Verdict;
use wsdbus_contract::wire::{self, WireDict};
use wsdbus_contract::{
    BUS_NAME, Change, Command, ErrorKind, FactKey, FactValue, Facts, OBJECT_PATH, Store, VERSION,
    Window, WindowAction, WorkspaceMode, admit,
};
use zbus::object_server::{InterfaceRef, SignalEmitter};
use zbus::{Connection, message::Header};

/// How the fake treats callers of mutating methods.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Callers {
    /// The sandbox check of the contract, as a real provider does.
    #[default]
    Check,
    /// Every caller is sandboxed: mutating methods return `Denied`.
    Deny,
}

#[derive(Clone, Debug)]
pub struct Config {
    /// Value of the `Provider` property.
    pub provider: String,
    pub capabilities: Vec<String>,
    pub callers: Callers,
}

impl Default for Config {
    /// Provider `fake` with every capability.
    fn default() -> Self {
        Self {
            provider: "fake".into(),
            capabilities: wsdbus_contract::all_capabilities(),
            callers: Callers::Check,
        }
    }
}

struct Service {
    config: Config,
    store: Store,
    commands: Vec<Command>,
    muted: usize,
}

async fn emit(emitter: &SignalEmitter<'_>, change: &Change) -> zbus::Result<()> {
    match change {
        Change::Facts {
            revision,
            changed,
            removed,
        } => {
            Service::state_changed(
                emitter,
                *revision,
                wire::encode_facts(changed),
                wire::encode_fact_names(removed),
            )
            .await
        }
        Change::Windows {
            revision,
            upserted,
            removed,
        } => {
            Service::windows_changed(
                emitter,
                *revision,
                wire::encode_windows(upserted),
                removed.clone(),
            )
            .await
        }
    }
}

impl Service {
    async fn publish(
        &mut self,
        emitter: &SignalEmitter<'_>,
        change: Option<Change>,
    ) -> zbus::Result<()> {
        let Some(change) = change else {
            return Ok(());
        };
        if self.muted > 0 {
            self.muted -= 1;
            return Ok(());
        }
        emit(emitter, &change).await
    }

    fn locked(&self) -> bool {
        self.store.facts().get(&FactKey::SessionLocked) == Some(&FactValue::Bool(true))
    }

    fn workspaces(&self) -> Vec<wsdbus_contract::Workspace> {
        match self.store.facts().get(&FactKey::Workspaces) {
            Some(FactValue::Workspaces(records)) => records.clone(),
            _ => Vec::new(),
        }
    }

    async fn execute(
        &mut self,
        command: Command,
        conn: &Connection,
        header: &Header<'_>,
        emitter: &SignalEmitter<'_>,
    ) -> Result<(), ProviderError> {
        let caller = match self.config.callers {
            Callers::Check => caller_verdict(conn, header).await,
            Callers::Deny => Verdict::Sandboxed(wsdbus_contract::sandbox::Sandbox::Flatpak),
        };
        admit(&command, &caller, &self.config.capabilities, self.locked())?;
        let change = self.perform(&command)?;
        self.commands.push(command);
        for change in change {
            // The state has changed either way; a lost signal shows up to
            // consumers as a revision gap.
            let _ = self.publish(emitter, Some(change)).await;
        }
        Ok(())
    }

    /// Applies the state effect of an admitted command.
    fn perform(&mut self, command: &Command) -> Result<Vec<Change>, Rejection> {
        let mut changes = Vec::new();
        match command {
            Command::Shortcut { .. } | Command::LaunchApp { .. } => {}
            Command::WindowAction { id, action } => {
                let Some(window) = self.store.window(id).cloned() else {
                    return Err(Rejection::new(
                        ErrorKind::StaleWindow,
                        format!("no window {id}"),
                    ));
                };
                let update = match action {
                    WindowAction::Close => self
                        .store
                        .update_windows(Vec::new(), std::slice::from_ref(id)),
                    WindowAction::Activate => {
                        let mut windows: Vec<Window> = self
                            .store
                            .windows()
                            .filter(|other| other.focused && other.id != *id)
                            .map(|other| Window {
                                focused: false,
                                ..other.clone()
                            })
                            .collect();
                        windows.push(Window {
                            focused: true,
                            minimized: false,
                            ..window
                        });
                        self.store.update_windows(windows, &[])
                    }
                    WindowAction::Minimize => self.store.update_windows(
                        vec![Window {
                            minimized: true,
                            focused: false,
                            ..window
                        }],
                        &[],
                    ),
                    WindowAction::Restore => self.store.update_windows(
                        vec![Window {
                            minimized: false,
                            ..window
                        }],
                        &[],
                    ),
                    WindowAction::ToggleMaximize => self.store.update_windows(
                        vec![Window {
                            maximized: !window.maximized,
                            ..window
                        }],
                        &[],
                    ),
                };
                changes.extend(update.expect("windows from the store are valid"));
            }
            Command::ActivateWorkspace { id } => {
                let records = self.known_workspace(id)?;
                let records = records
                    .into_iter()
                    .map(|record| wsdbus_contract::Workspace {
                        active: record.id == *id,
                        ..record
                    })
                    .collect();
                let facts = Facts::from([
                    (FactKey::Workspaces, FactValue::Workspaces(records)),
                    (FactKey::WorkspaceActive, FactValue::Text(id.clone())),
                ]);
                changes.extend(
                    self.store
                        .update_facts(facts, &[])
                        .expect("valid workspaces"),
                );
            }
            Command::SetWorkspaceMode { id, mode } => {
                let records = self.known_workspace(id)?;
                let records = records
                    .into_iter()
                    .map(|record| {
                        let mode = if record.id == *id {
                            Some(*mode)
                        } else {
                            record.mode
                        };
                        wsdbus_contract::Workspace { mode, ..record }
                    })
                    .collect();
                let facts = Facts::from([(FactKey::Workspaces, FactValue::Workspaces(records))]);
                changes.extend(
                    self.store
                        .update_facts(facts, &[])
                        .expect("valid workspaces"),
                );
            }
        }
        Ok(changes)
    }

    fn known_workspace(&self, id: &str) -> Result<Vec<wsdbus_contract::Workspace>, Rejection> {
        let records = self.workspaces();
        if records.iter().any(|record| record.id == id) {
            Ok(records)
        } else {
            Err(Rejection::new(
                ErrorKind::InvalidArgs,
                format!("no workspace {id}"),
            ))
        }
    }
}

fn parse<T>(value: Option<T>, what: &str, name: &str) -> Result<T, ProviderError> {
    value.ok_or_else(|| {
        Rejection::new(ErrorKind::InvalidArgs, format!("unknown {what} {name}")).into()
    })
}

#[zbus::interface(name = "io.github.kkkingqz.WsDbus.Desktop1")]
impl Service {
    #[zbus(property)]
    fn version(&self) -> u32 {
        VERSION
    }

    #[zbus(property)]
    fn provider(&self) -> String {
        self.config.provider.clone()
    }

    #[zbus(property)]
    fn capabilities(&self) -> Vec<String> {
        self.config.capabilities.clone()
    }

    #[zbus(out_args("revision", "facts"))]
    fn get_state(&self) -> (u64, WireDict) {
        (
            self.store.revision(),
            wire::encode_facts(self.store.facts()),
        )
    }

    #[zbus(out_args("revision", "windows"))]
    fn get_windows(&self) -> (u64, Vec<WireDict>) {
        let windows: Vec<Window> = self.store.windows().cloned().collect();
        (self.store.revision(), wire::encode_windows(&windows))
    }

    async fn shortcut(
        &mut self,
        keys: Vec<String>,
        app: String,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), ProviderError> {
        let app = (!app.is_empty()).then_some(app);
        self.execute(Command::Shortcut { keys, app }, conn, &header, &emitter)
            .await
    }

    async fn launch_app(
        &mut self,
        id: String,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), ProviderError> {
        self.execute(Command::LaunchApp { id }, conn, &header, &emitter)
            .await
    }

    async fn activate_workspace(
        &mut self,
        id: String,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), ProviderError> {
        self.execute(Command::ActivateWorkspace { id }, conn, &header, &emitter)
            .await
    }

    async fn window_action(
        &mut self,
        id: String,
        action: String,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), ProviderError> {
        let action = parse(WindowAction::from_name(&action), "action", &action)?;
        self.execute(
            Command::WindowAction { id, action },
            conn,
            &header,
            &emitter,
        )
        .await
    }

    async fn set_workspace_mode(
        &mut self,
        id: String,
        mode: String,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), ProviderError> {
        let mode = parse(WorkspaceMode::from_name(&mode), "mode", &mode)?;
        self.execute(
            Command::SetWorkspaceMode { id, mode },
            conn,
            &header,
            &emitter,
        )
        .await
    }

    #[zbus(signal)]
    async fn state_changed(
        emitter: &SignalEmitter<'_>,
        revision: u64,
        changed: WireDict,
        removed: Vec<String>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn windows_changed(
        emitter: &SignalEmitter<'_>,
        revision: u64,
        upserted: Vec<WireDict>,
        removed: Vec<String>,
    ) -> zbus::Result<()>;
}

/// Handle of a served fake provider.
#[derive(Clone)]
pub struct Fake {
    conn: Connection,
    service: InterfaceRef<Service>,
}

impl Fake {
    /// Serves the fake at the contract path and takes the bus name.
    pub async fn serve(conn: &Connection, config: Config) -> zbus::Result<Self> {
        let service = Service {
            config,
            store: Store::new(),
            commands: Vec::new(),
            muted: 0,
        };
        conn.object_server().at(OBJECT_PATH, service).await?;
        let service = conn
            .object_server()
            .interface::<_, Service>(OBJECT_PATH)
            .await?;
        conn.request_name(BUS_NAME).await?;
        Ok(Self {
            conn: conn.clone(),
            service,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Changes facts and emits `StateChanged`. Returns the new revision, or
    /// `None` when nothing changed.
    pub async fn update_facts(
        &self,
        changed: Facts,
        removed: &[FactKey],
    ) -> zbus::Result<Option<u64>> {
        let mut service = self.service.get_mut().await;
        let change = service
            .store
            .update_facts(changed, removed)
            .map_err(|error| zbus::Error::Failure(error.to_string()))?;
        let revision = change.as_ref().map(Change::revision);
        service
            .publish(self.service.signal_emitter(), change)
            .await?;
        Ok(revision)
    }

    /// Changes windows and emits `WindowsChanged`.
    pub async fn update_windows(
        &self,
        upserted: Vec<Window>,
        removed: &[String],
    ) -> zbus::Result<Option<u64>> {
        let mut service = self.service.get_mut().await;
        let change = service
            .store
            .update_windows(upserted, removed)
            .map_err(|error| zbus::Error::Failure(error.to_string()))?;
        let revision = change.as_ref().map(Change::revision);
        service
            .publish(self.service.signal_emitter(), change)
            .await?;
        Ok(revision)
    }

    /// The next `count` changes are applied without their signals, so
    /// consumers see a revision gap.
    pub async fn mute(&self, count: usize) {
        self.service.get_mut().await.muted += count;
    }

    /// Emits a change as is, without touching the state; for testing how
    /// consumers treat stale and malformed signals.
    pub async fn emit_raw(&self, change: &Change) -> zbus::Result<()> {
        emit(self.service.signal_emitter(), change).await
    }

    /// Commands that passed the checks, oldest first.
    pub async fn commands(&self) -> Vec<Command> {
        self.service.get().await.commands.clone()
    }

    pub async fn revision(&self) -> u64 {
        self.service.get().await.store.revision()
    }

    /// Releases the bus name and removes the object.
    pub async fn stop(self) -> zbus::Result<()> {
        self.conn.release_name(BUS_NAME).await?;
        self.conn
            .object_server()
            .remove::<Service, _>(OBJECT_PATH)
            .await?;
        Ok(())
    }
}
