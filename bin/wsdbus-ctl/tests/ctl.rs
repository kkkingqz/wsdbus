//! `wsdbus-ctl` against the fake provider on a private bus.

use std::io::{BufRead, BufReader};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use wsdbus_contract::{FactKey, FactValue, Facts, Window};
use wsdbus_fake::bus::PrivateBus;
use wsdbus_fake::{Callers, Config, Fake};

fn run<T>(test: impl Future<Output = T>) -> T {
    async_io::block_on(futures_lite::future::or(test, async {
        async_io::Timer::after(Duration::from_secs(10)).await;
        panic!("test timed out")
    }))
}

fn ctl(bus: &PrivateBus) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wsdbus-ctl"));
    command.env("DBUS_SESSION_BUS_ADDRESS", bus.address());
    command
}

fn output(bus: &PrivateBus, args: &[&str]) -> Output {
    ctl(bus).args(args).output().unwrap()
}

fn status(bus: &PrivateBus, args: &[&str]) -> i32 {
    output(bus, args).status.code().unwrap()
}

fn json_line(bus: &PrivateBus, args: &[&str]) -> Value {
    let output = output(bus, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

async fn fake(bus: &PrivateBus, config: Config) -> Fake {
    let fake = Fake::serve(&bus.connect().await.unwrap(), config)
        .await
        .unwrap();
    fake.update_facts(
        Facts::from([
            (FactKey::SessionLocked, FactValue::Bool(false)),
            (FactKey::AppearanceScheme, FactValue::Text("dark".into())),
        ]),
        &[],
    )
    .await
    .unwrap();
    let window = Window {
        id: "1".into(),
        app: "org.gnome.Ptyxis".into(),
        title: "Terminal".into(),
        ..Window::default()
    };
    fake.update_windows(vec![window], &[]).await.unwrap();
    fake
}

#[test]
fn queries() {
    run(async {
        let bus = PrivateBus::start().unwrap();
        let _fake = fake(&bus, Config::default()).await;
        assert_eq!(
            json_line(&bus, &["--json", "state"]),
            json!({"revision": 2, "facts": {"appearance.scheme": "dark", "session.locked": false}})
        );
        let windows = json_line(&bus, &["windows", "--json"]);
        assert_eq!(windows["windows"][0]["title"], "Terminal");
        let info = json_line(&bus, &["info", "--json"]);
        assert_eq!(info["provider"], "fake");
        let text = String::from_utf8(output(&bus, &["state"]).stdout).unwrap();
        assert!(text.contains("appearance.scheme = \"dark\""), "{text}");
    });
}

#[test]
fn exit_codes() {
    run(async {
        let bus = PrivateBus::start().unwrap();
        assert_eq!(status(&bus, &["launch", "org.gnome.Nautilus"]), 3);
        assert_eq!(status(&bus, &["state"]), 3);
        assert_eq!(status(&bus, &["frobnicate"]), 2);
        assert_eq!(status(&bus, &["window", "1", "explode"]), 2);
        assert_eq!(status(&bus, &["--help"]), 0);

        let capabilities = wsdbus_contract::all_capabilities()
            .into_iter()
            .filter(|capability| capability != "SetWorkspaceMode")
            .collect();
        let first = fake(
            &bus,
            Config {
                capabilities,
                ..Config::default()
            },
        )
        .await;
        assert_eq!(status(&bus, &["launch", "org.gnome.Nautilus"]), 0);
        assert_eq!(
            status(
                &bus,
                &["shortcut", "--app", "org.gnome.Ptyxis", "Control_L", "t"]
            ),
            0
        );
        assert_eq!(status(&bus, &["workspace", "9"]), 2);
        assert_eq!(status(&bus, &["mode", "1", "floating"]), 4);
        assert_eq!(status(&bus, &["window", "9", "close"]), 5);
        first
            .update_facts(
                Facts::from([(FactKey::SessionLocked, FactValue::Bool(true))]),
                &[],
            )
            .await
            .unwrap();
        assert_eq!(status(&bus, &["launch", "org.gnome.Nautilus"]), 6);
        assert_eq!(status(&bus, &["window", "1", "minimize"]), 0);
        first.stop().await.unwrap();

        let _second = fake(
            &bus,
            Config {
                callers: Callers::Deny,
                ..Config::default()
            },
        )
        .await;
        assert_eq!(status(&bus, &["window", "1", "close"]), 7);
    });
}

#[test]
fn watch_json() {
    run(async {
        let bus = PrivateBus::start().unwrap();
        let fake = fake(&bus, Config::default()).await;
        let mut child = ctl(&bus)
            .args(["watch", "--json"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut next =
            || -> Value { serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap() };
        let snapshot = next();
        assert_eq!(snapshot["snapshot"], true);
        assert_eq!(snapshot["revision"], 2);
        assert_eq!(snapshot["facts"]["appearance.scheme"], "dark");
        assert_eq!(snapshot["windows"][0]["id"], "1");

        fake.update_facts(
            Facts::from([(FactKey::SessionLocked, FactValue::Bool(true))]),
            &[],
        )
        .await
        .unwrap();
        assert_eq!(
            next(),
            json!({"revision": 3, "facts": {"session.locked": true}, "removed": []})
        );
        fake.update_windows(Vec::new(), &["1".into()])
            .await
            .unwrap();
        assert_eq!(
            next(),
            json!({"revision": 4, "windows": [], "removed": ["1"]})
        );
        fake.stop().await.unwrap();
        assert_eq!(next(), json!({"lost": true}));
        let _again = self::fake(&bus, Config::default()).await;
        assert_eq!(next()["snapshot"], true);
        child.kill().unwrap();
        child.wait().unwrap();
    });
}
