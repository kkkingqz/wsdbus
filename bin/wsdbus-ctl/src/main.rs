//! Command-line client of the wsdbus `Desktop1` contract.

use std::io::Write;
use std::process::ExitCode;

use serde_json::{Value, json};
use wsdbus_client::{Client, Error, Snapshot, Update};
use wsdbus_contract::json as contract_json;
use wsdbus_contract::{Change, Command, ErrorKind, Facts, Window, WindowAction, WorkspaceMode};

const USAGE: &str = "\
Usage: wsdbus-ctl [--json] COMMAND

Queries:
  watch                    print a snapshot, then every change, until stopped
  state                    print the facts
  windows                  print the windows
  info                     print the provider, version and capabilities

Commands:
  shortcut [--app APP] KEY...  press keys (XKB keysym names), only while APP
                               is in focus when --app is given
  launch APP                   launch an app by its app id
  workspace ID                 switch to a workspace
  window ID ACTION             activate, close, minimize, restore or
                               toggle-maximize a window
  mode WORKSPACE MODE          set a workspace to tiling or floating

Options:
  --json   one JSON object per line
  -h, --help

Exit status: 0 success, 1 bus or protocol error, 2 usage or invalid
arguments, 3 no provider, 4 unsupported, 5 stale window, 6 locked,
7 denied.";

const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NO_PROVIDER: u8 = 3;

fn exit_code(error: &Error) -> u8 {
    match error {
        Error::NoProvider => EXIT_NO_PROVIDER,
        Error::Rejected { kind, .. } => match kind {
            ErrorKind::InvalidArgs => EXIT_USAGE,
            ErrorKind::Unsupported => 4,
            ErrorKind::StaleWindow => 5,
            ErrorKind::Locked => 6,
            ErrorKind::Denied => 7,
        },
        Error::Protocol(_) | Error::Bus(_) => EXIT_FAILURE,
    }
}

enum Query {
    Watch,
    State,
    Windows,
    Info,
}

enum Request {
    Query(Query),
    Command(Command),
}

struct Usage(String);

fn parse(args: &[String]) -> Result<(Request, bool), Usage> {
    let mut json = false;
    let mut app = None;
    let mut words = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--app" => {
                app = Some(
                    iter.next()
                        .ok_or(Usage("--app needs a value".into()))?
                        .clone(),
                )
            }
            "--" => words.extend(iter.by_ref().cloned()),
            option if option.starts_with("--") => {
                return Err(Usage(format!("unknown option {option}")));
            }
            word => words.push(word.to_owned()),
        }
    }
    let (name, rest) = words.split_first().ok_or(Usage("missing command".into()))?;
    let exact = |count: usize| {
        if rest.len() == count {
            Ok(())
        } else {
            Err(Usage(format!("{name} takes {count} argument(s)")))
        }
    };
    if app.is_some() && name != "shortcut" {
        return Err(Usage("--app is only for shortcut".into()));
    }
    let request = match name.as_str() {
        "watch" | "state" | "windows" | "info" => {
            exact(0)?;
            Request::Query(match name.as_str() {
                "watch" => Query::Watch,
                "state" => Query::State,
                "windows" => Query::Windows,
                _ => Query::Info,
            })
        }
        "shortcut" => {
            if rest.is_empty() {
                return Err(Usage("shortcut needs keys".into()));
            }
            Request::Command(Command::Shortcut {
                keys: rest.to_vec(),
                app,
            })
        }
        "launch" => {
            exact(1)?;
            Request::Command(Command::LaunchApp {
                id: rest[0].clone(),
            })
        }
        "workspace" => {
            exact(1)?;
            Request::Command(Command::ActivateWorkspace {
                id: rest[0].clone(),
            })
        }
        "window" => {
            exact(2)?;
            let action = WindowAction::from_name(&rest[1])
                .ok_or_else(|| Usage(format!("unknown window action {}", rest[1])))?;
            Request::Command(Command::WindowAction {
                id: rest[0].clone(),
                action,
            })
        }
        "mode" => {
            exact(2)?;
            let mode = WorkspaceMode::from_name(&rest[1])
                .ok_or_else(|| Usage(format!("unknown mode {}", rest[1])))?;
            Request::Command(Command::SetWorkspaceMode {
                id: rest[0].clone(),
                mode,
            })
        }
        other => return Err(Usage(format!("unknown command {other}"))),
    };
    Ok((request, json))
}

/// Writes one line and flushes it; a closed stdout ends the program quietly.
fn emit(line: &str) {
    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        std::process::exit(0);
    }
}

fn warn_rejected(rejected: &[String]) {
    if !rejected.is_empty() {
        eprintln!(
            "wsdbus-ctl: dropped invalid entries: {}",
            rejected.join(", ")
        );
    }
}

fn fact_lines(facts: &Facts, prefix: &str) {
    for (key, value) in facts {
        emit(&format!(
            "{prefix}{key} = {}",
            contract_json::value_to_json(value)
        ));
    }
}

fn window_line(window: &Window, prefix: &str) {
    let flags: Vec<&str> = [
        (window.focused, "focused"),
        (window.minimized, "minimized"),
        (window.maximized, "maximized"),
        (window.floating, "floating"),
        (window.fullscreen, "fullscreen"),
    ]
    .into_iter()
    .filter_map(|(set, name)| set.then_some(name))
    .collect();
    emit(&format!(
        "{prefix}{}\t{}\t{}\t{}\t{}",
        window.id,
        window.app,
        window.workspace,
        if flags.is_empty() {
            "-".into()
        } else {
            flags.join(",")
        },
        window.title
    ));
}

fn print_snapshot(snapshot: &Snapshot, json: bool) {
    warn_rejected(&snapshot.rejected);
    if json {
        emit(
            &json!({
                "snapshot": true,
                "revision": snapshot.revision,
                "provider": snapshot.info.provider,
                "version": snapshot.info.version,
                "capabilities": snapshot.info.capabilities,
                "facts": contract_json::facts_to_json(&snapshot.facts),
                "windows": contract_json::windows_to_json(&snapshot.windows),
            })
            .to_string(),
        );
        return;
    }
    emit(&format!(
        "snapshot {} provider {} version {}",
        snapshot.revision, snapshot.info.provider, snapshot.info.version
    ));
    fact_lines(&snapshot.facts, "");
    for window in &snapshot.windows {
        window_line(window, "window ");
    }
}

fn print_change(change: &Change, json: bool) {
    if json {
        emit(&contract_json::change_to_json(change).to_string());
        return;
    }
    let revision = change.revision();
    match change {
        Change::Facts {
            changed, removed, ..
        } => {
            fact_lines(changed, &format!("{revision} "));
            for key in removed {
                emit(&format!("{revision} -{key}"));
            }
        }
        Change::Windows {
            upserted, removed, ..
        } => {
            for window in upserted {
                window_line(window, &format!("{revision} window "));
            }
            for id in removed {
                emit(&format!("{revision} -window {id}"));
            }
        }
    }
}

async fn watch(client: &Client, json: bool) -> Result<(), Error> {
    let mut watcher = client.watch().await?;
    loop {
        match watcher.next().await? {
            Update::Snapshot(snapshot) => print_snapshot(&snapshot, json),
            Update::Change { change, rejected } => {
                warn_rejected(&rejected);
                print_change(&change, json);
            }
            Update::Lost => emit(if json { r#"{"lost":true}"# } else { "lost" }),
        }
    }
}

async fn query(client: &Client, query: Query, json: bool) -> Result<(), Error> {
    match query {
        Query::Watch => watch(client, json).await,
        Query::State => {
            let (revision, facts) = client.state().await?;
            warn_rejected(&facts.rejected);
            if json {
                emit(&json!({"revision": revision, "facts": contract_json::facts_to_json(&facts.value)}).to_string());
            } else {
                emit(&format!("revision {revision}"));
                fact_lines(&facts.value, "");
            }
            Ok(())
        }
        Query::Windows => {
            let (revision, windows) = client.windows().await?;
            warn_rejected(&windows.rejected);
            if json {
                emit(&json!({"revision": revision, "windows": contract_json::windows_to_json(&windows.value)}).to_string());
            } else {
                emit(&format!("revision {revision}"));
                for window in &windows.value {
                    window_line(window, "");
                }
            }
            Ok(())
        }
        Query::Info => {
            let info = client.info().await?;
            if json {
                let value: Value = json!({
                    "provider": info.provider,
                    "version": info.version,
                    "capabilities": info.capabilities,
                });
                emit(&value.to_string());
            } else {
                emit(&format!("provider {}", info.provider));
                emit(&format!("version {}", info.version));
                emit(&format!("capabilities {}", info.capabilities.join(" ")));
            }
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (request, json) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(Usage(message)) => {
            eprintln!("wsdbus-ctl: {message}\n\n{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let result = async_io::block_on(async {
        let client = Client::session().await.map_err(Error::Bus)?;
        match request {
            Request::Query(name) => query(&client, name, json).await,
            Request::Command(command) => client.run(&command).await,
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("wsdbus-ctl: {error}");
            ExitCode::from(exit_code(&error))
        }
    }
}
