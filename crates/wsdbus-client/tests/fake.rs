//! The client against the fake provider on a private bus.

use std::time::Duration;

use wsdbus_client::{Client, Error, Update, Watcher};
use wsdbus_contract::wire;
use wsdbus_contract::{
    Change, Command, ErrorKind, FactKey, FactValue, Facts, INTERFACE, OBJECT_PATH, Window,
    WindowAction, Workspace, WorkspaceMode,
};
use wsdbus_fake::bus::PrivateBus;
use wsdbus_fake::{Callers, Config, Fake};

fn run<T>(test: impl Future<Output = T>) -> T {
    async_io::block_on(futures_lite::future::or(test, async {
        async_io::Timer::after(Duration::from_secs(10)).await;
        panic!("test timed out")
    }))
}

fn start() -> PrivateBus {
    PrivateBus::start().expect("dbus-daemon must be installed for these tests")
}

fn text(value: &str) -> FactValue {
    FactValue::Text(value.into())
}

fn window(id: &str, app: &str) -> Window {
    Window {
        id: id.into(),
        app: app.into(),
        class: app.into(),
        title: format!("{app} {id}"),
        workspace: "1".into(),
        monitor: "eDP-1".into(),
        ..Window::default()
    }
}

fn workspaces() -> FactValue {
    let workspace = |id: &str, active| Workspace {
        id: id.into(),
        name: id.into(),
        monitor: "eDP-1".into(),
        active,
        mode: Some(WorkspaceMode::Tiling),
    };
    FactValue::Workspaces(vec![workspace("1", true), workspace("2", false)])
}

async fn fake(bus: &PrivateBus, config: Config) -> Fake {
    let fake = Fake::serve(&bus.connect().await.unwrap(), config)
        .await
        .unwrap();
    fake.update_facts(
        Facts::from([
            (FactKey::FocusApp, text("org.gnome.Ptyxis")),
            (FactKey::SessionLocked, FactValue::Bool(false)),
            (FactKey::Workspaces, workspaces()),
            (FactKey::WorkspaceActive, text("1")),
        ]),
        &[],
    )
    .await
    .unwrap();
    fake.update_windows(
        vec![window("1", "org.gnome.Ptyxis"), window("2", "firefox")],
        &[],
    )
    .await
    .unwrap();
    fake
}

async fn client(bus: &PrivateBus) -> Client {
    Client::new(bus.connect().await.unwrap())
}

async fn snapshot(watcher: &mut Watcher) -> wsdbus_client::Snapshot {
    match watcher.next().await.unwrap() {
        Update::Snapshot(snapshot) => snapshot,
        other => panic!("expected a snapshot, got {other:?}"),
    }
}

async fn change(watcher: &mut Watcher) -> Change {
    match watcher.next().await.unwrap() {
        Update::Change { change, rejected } => {
            assert!(rejected.is_empty());
            change
        }
        other => panic!("expected a change, got {other:?}"),
    }
}

#[test]
fn snapshot_then_changes() {
    run(async {
        let bus = start();
        let fake = fake(&bus, Config::default()).await;
        let mut watcher = client(&bus).await.watch().await.unwrap();
        let first = snapshot(&mut watcher).await;
        assert_eq!(first.info.provider, "fake");
        assert_eq!(first.info.version, 1);
        assert_eq!(first.revision, 2);
        assert_eq!(first.facts[&FactKey::FocusApp], text("org.gnome.Ptyxis"));
        assert_eq!(first.windows.len(), 2);

        fake.update_facts(
            Facts::from([(FactKey::FocusApp, text("firefox"))]),
            &[FactKey::WorkspaceActive],
        )
        .await
        .unwrap();
        assert_eq!(
            change(&mut watcher).await,
            Change::Facts {
                revision: 3,
                changed: Facts::from([(FactKey::FocusApp, text("firefox"))]),
                removed: vec![FactKey::WorkspaceActive],
            }
        );
        fake.update_windows(vec![window("3", "foot")], &["1".into()])
            .await
            .unwrap();
        assert_eq!(
            change(&mut watcher).await,
            Change::Windows {
                revision: 4,
                upserted: vec![window("3", "foot")],
                removed: vec!["1".into()],
            }
        );
        let ids: Vec<_> = watcher
            .mirror()
            .windows()
            .map(|window| window.id.as_str())
            .collect();
        assert_eq!(ids, ["2", "3"]);
        assert!(
            !watcher
                .mirror()
                .facts()
                .contains_key(&FactKey::WorkspaceActive)
        );
    });
}

#[test]
fn gap_takes_a_new_snapshot() {
    run(async {
        let bus = start();
        let fake = fake(&bus, Config::default()).await;
        let mut watcher = client(&bus).await.watch().await.unwrap();
        snapshot(&mut watcher).await;
        fake.mute(1).await;
        fake.update_facts(Facts::from([(FactKey::FocusApp, text("muted"))]), &[])
            .await
            .unwrap();
        fake.update_facts(Facts::from([(FactKey::FocusClass, text("next"))]), &[])
            .await
            .unwrap();
        let again = snapshot(&mut watcher).await;
        assert_eq!(again.revision, 4);
        assert_eq!(again.facts[&FactKey::FocusApp], text("muted"));
        assert_eq!(again.facts[&FactKey::FocusClass], text("next"));
    });
}

#[test]
fn stale_and_foreign_signals_are_ignored() {
    run(async {
        let bus = start();
        let fake = fake(&bus, Config::default()).await;
        let mut watcher = client(&bus).await.watch().await.unwrap();
        snapshot(&mut watcher).await;
        let stale = Change::Facts {
            revision: 1,
            changed: Facts::from([(FactKey::FocusApp, text("stale"))]),
            removed: vec![],
        };
        fake.emit_raw(&stale).await.unwrap();
        let foreign = bus.connect().await.unwrap();
        let forged = Facts::from([(FactKey::SessionLocked, FactValue::Bool(true))]);
        foreign
            .emit_signal(
                None::<&str>,
                OBJECT_PATH,
                INTERFACE,
                "StateChanged",
                &(3u64, wire::encode_facts(&forged), Vec::<String>::new()),
            )
            .await
            .unwrap();
        fake.update_facts(Facts::from([(FactKey::FocusApp, text("real"))]), &[])
            .await
            .unwrap();
        assert_eq!(
            change(&mut watcher).await,
            Change::Facts {
                revision: 3,
                changed: Facts::from([(FactKey::FocusApp, text("real"))]),
                removed: vec![],
            }
        );
        assert_eq!(
            watcher.mirror().facts()[&FactKey::SessionLocked],
            FactValue::Bool(false)
        );
    });
}

#[test]
fn provider_restart_and_late_start() {
    run(async {
        let bus = start();
        let mut watcher = client(&bus).await.watch().await.unwrap();
        let first = fake(&bus, Config::default()).await;
        assert_eq!(snapshot(&mut watcher).await.revision, 2);
        first.stop().await.unwrap();
        assert_eq!(watcher.next().await.unwrap(), Update::Lost);
        assert!(watcher.owner().is_none());
        let second = fake(
            &bus,
            Config {
                provider: "second".into(),
                ..Config::default()
            },
        )
        .await;
        let again = snapshot(&mut watcher).await;
        assert_eq!(again.info.provider, "second");
        second
            .update_facts(Facts::from([(FactKey::FocusApp, text("after"))]), &[])
            .await
            .unwrap();
        assert_eq!(change(&mut watcher).await.revision(), 3);
    });
}

async fn rejected(client: &Client, command: Command) -> Option<ErrorKind> {
    match client.run(&command).await {
        Ok(()) => None,
        Err(Error::Rejected { kind, .. }) => Some(kind),
        Err(other) => panic!("unexpected error {other}"),
    }
}

#[test]
fn commands_and_errors() {
    run(async {
        let bus = start();
        let fake = fake(&bus, Config::default()).await;
        let client = client(&bus).await;
        let launch = Command::LaunchApp {
            id: "org.gnome.Nautilus".into(),
        };
        let shortcut = Command::Shortcut {
            keys: vec!["Control_L".into(), "t".into()],
            app: Some("org.gnome.Ptyxis".into()),
        };
        assert_eq!(rejected(&client, launch.clone()).await, None);
        assert_eq!(rejected(&client, shortcut.clone()).await, None);
        let close_missing = Command::WindowAction {
            id: "9".into(),
            action: WindowAction::Close,
        };
        assert_eq!(
            rejected(&client, close_missing).await,
            Some(ErrorKind::StaleWindow)
        );
        let empty_keys = Command::Shortcut {
            keys: vec![],
            app: None,
        };
        assert_eq!(
            rejected(&client, empty_keys).await,
            Some(ErrorKind::InvalidArgs)
        );
        let unknown_workspace = Command::ActivateWorkspace { id: "7".into() };
        assert_eq!(
            rejected(&client, unknown_workspace).await,
            Some(ErrorKind::InvalidArgs)
        );

        fake.update_facts(
            Facts::from([(FactKey::SessionLocked, FactValue::Bool(true))]),
            &[],
        )
        .await
        .unwrap();
        assert_eq!(
            rejected(&client, shortcut.clone()).await,
            Some(ErrorKind::Locked)
        );
        assert_eq!(
            rejected(&client, launch.clone()).await,
            Some(ErrorKind::Locked)
        );
        let minimize = Command::WindowAction {
            id: "1".into(),
            action: WindowAction::Minimize,
        };
        assert_eq!(rejected(&client, minimize.clone()).await, None);
        assert_eq!(fake.commands().await, [launch, shortcut, minimize]);
    });
}

#[test]
fn unsupported_denied_and_no_provider() {
    run(async {
        let bus = start();
        let client = client(&bus).await;
        let launch = Command::LaunchApp {
            id: "org.gnome.Nautilus".into(),
        };
        assert!(matches!(client.run(&launch).await, Err(Error::NoProvider)));
        assert!(matches!(client.state().await, Err(Error::NoProvider)));

        let capabilities = wsdbus_contract::all_capabilities()
            .into_iter()
            .filter(|capability| capability != "SetWorkspaceMode")
            .collect();
        let fake = fake(
            &bus,
            Config {
                capabilities,
                ..Config::default()
            },
        )
        .await;
        let mode = Command::SetWorkspaceMode {
            id: "1".into(),
            mode: WorkspaceMode::Floating,
        };
        assert_eq!(rejected(&client, mode).await, Some(ErrorKind::Unsupported));
        assert!(!client.info().await.unwrap().supports("SetWorkspaceMode"));
        fake.stop().await.unwrap();

        let fake = fake_denying(&bus).await;
        assert_eq!(rejected(&client, launch).await, Some(ErrorKind::Denied));
        assert!(fake.commands().await.is_empty());
        assert_eq!(client.state().await.unwrap().0, 2);
    });
}

async fn fake_denying(bus: &PrivateBus) -> Fake {
    fake(
        bus,
        Config {
            callers: Callers::Deny,
            ..Config::default()
        },
    )
    .await
}

#[test]
fn command_effects_reach_the_watcher() {
    run(async {
        let bus = start();
        let _fake = fake(&bus, Config::default()).await;
        let client = client(&bus).await;
        let mut watcher = client.watch().await.unwrap();
        snapshot(&mut watcher).await;
        let minimize = Command::WindowAction {
            id: "2".into(),
            action: WindowAction::Minimize,
        };
        client.run(&minimize).await.unwrap();
        let Change::Windows { upserted, .. } = change(&mut watcher).await else {
            panic!("expected a windows change");
        };
        assert!(upserted[0].minimized);
        client
            .run(&Command::ActivateWorkspace { id: "2".into() })
            .await
            .unwrap();
        let Change::Facts { changed, .. } = change(&mut watcher).await else {
            panic!("expected a facts change");
        };
        assert_eq!(changed[&FactKey::WorkspaceActive], text("2"));
        client
            .run(&Command::SetWorkspaceMode {
                id: "2".into(),
                mode: WorkspaceMode::Floating,
            })
            .await
            .unwrap();
        let Change::Facts { changed, .. } = change(&mut watcher).await else {
            panic!("expected a facts change");
        };
        let FactValue::Workspaces(records) = &changed[&FactKey::Workspaces] else {
            panic!("expected workspaces");
        };
        assert_eq!(records[1].mode, Some(WorkspaceMode::Floating));
        client
            .run(&Command::WindowAction {
                id: "1".into(),
                action: WindowAction::Close,
            })
            .await
            .unwrap();
        let Change::Windows { removed, .. } = change(&mut watcher).await else {
            panic!("expected a windows change");
        };
        assert_eq!(removed, ["1"]);
    });
}
