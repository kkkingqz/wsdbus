//! JSON encoding used by `wsdbus-ctl` and the fake provider's input.
//!
//! Facts are an object keyed by fact name; windows are objects with the
//! window keys. Parsing is strict: unknown names and wrong values are errors.

use serde_json::{Map, Value, json};

use crate::fact::{self, FactKey, FactValue, Facts, InvalidFact, Workspace, WorkspaceMode};
use crate::store::Change;
use crate::window::Window;

fn workspace_to_json(workspace: &Workspace) -> Value {
    let mut object = json!({
        "id": workspace.id,
        "name": workspace.name,
        "monitor": workspace.monitor,
        "active": workspace.active,
    });
    if let Some(mode) = workspace.mode {
        object["mode"] = mode.name().into();
    }
    object
}

pub fn value_to_json(value: &FactValue) -> Value {
    match value {
        FactValue::Text(text) => text.as_str().into(),
        FactValue::Bool(flag) => (*flag).into(),
        FactValue::Number(number) => (*number).into(),
        FactValue::Workspaces(records) => records.iter().map(workspace_to_json).collect(),
    }
}

pub fn facts_to_json(facts: &Facts) -> Value {
    Value::Object(
        facts
            .iter()
            .map(|(key, value)| (key.name().to_owned(), value_to_json(value)))
            .collect(),
    )
}

fn object_text(object: &Map<String, Value>, name: &str) -> Result<String, String> {
    match object.get(name) {
        None => Ok(String::new()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(format!("{name} must be a string")),
    }
}

fn object_flag(object: &Map<String, Value>, name: &str) -> Result<bool, String> {
    match object.get(name) {
        None => Ok(false),
        Some(Value::Bool(flag)) => Ok(*flag),
        Some(_) => Err(format!("{name} must be a boolean")),
    }
}

fn check_keys(object: &Map<String, Value>, known: &[&str]) -> Result<(), String> {
    match object.keys().find(|key| !known.contains(&key.as_str())) {
        Some(key) => Err(format!("unknown key {key}")),
        None => Ok(()),
    }
}

fn workspace_from_json(value: &Value) -> Result<Workspace, String> {
    let object = value.as_object().ok_or("workspace must be an object")?;
    check_keys(object, &["id", "name", "monitor", "active", "mode"])?;
    let mode = match object.get("mode") {
        None => None,
        Some(Value::String(name)) => {
            Some(WorkspaceMode::from_name(name).ok_or_else(|| format!("unknown mode {name}"))?)
        }
        Some(_) => return Err("mode must be a string".into()),
    };
    Ok(Workspace {
        id: object_text(object, "id")?,
        name: object_text(object, "name")?,
        monitor: object_text(object, "monitor")?,
        active: object_flag(object, "active")?,
        mode,
    })
}

pub fn value_from_json(key: FactKey, value: &Value) -> Result<FactValue, InvalidFact> {
    let fail = |reason: String| InvalidFact::new(key.name(), reason);
    let value = match (key.kind(), value) {
        (fact::ValueKind::Text, Value::String(text)) => FactValue::Text(text.clone()),
        (fact::ValueKind::Bool, Value::Bool(flag)) => FactValue::Bool(*flag),
        (fact::ValueKind::Number, Value::Number(number)) => {
            FactValue::Number(number.as_f64().ok_or_else(|| fail("not a number".into()))?)
        }
        (fact::ValueKind::Workspaces, Value::Array(records)) => FactValue::Workspaces(
            records
                .iter()
                .map(workspace_from_json)
                .collect::<Result<_, _>>()
                .map_err(fail)?,
        ),
        _ => return Err(fail("wrong value type".into())),
    };
    fact::validate(key, &value)?;
    Ok(value)
}

pub fn facts_from_json(object: &Map<String, Value>) -> Result<Facts, InvalidFact> {
    object
        .iter()
        .map(|(name, value)| {
            let key = FactKey::from_name(name)
                .ok_or_else(|| InvalidFact::new(name.as_str(), "unknown fact"))?;
            Ok((key, value_from_json(key, value)?))
        })
        .collect()
}

pub fn fact_names_from_json(value: &Value) -> Result<Vec<FactKey>, InvalidFact> {
    let names = value
        .as_array()
        .ok_or_else(|| InvalidFact::new("removed", "expected an array of fact names"))?;
    names
        .iter()
        .map(|name| {
            name.as_str()
                .and_then(FactKey::from_name)
                .ok_or_else(|| InvalidFact::new(name.to_string(), "unknown fact"))
        })
        .collect()
}

pub fn window_to_json(window: &Window) -> Value {
    json!({
        "id": window.id,
        "app": window.app,
        "class": window.class,
        "title": window.title,
        "workspace": window.workspace,
        "monitor": window.monitor,
        "focused": window.focused,
        "minimized": window.minimized,
        "maximized": window.maximized,
        "floating": window.floating,
        "fullscreen": window.fullscreen,
    })
}

pub fn windows_to_json<'a>(windows: impl IntoIterator<Item = &'a Window>) -> Value {
    windows.into_iter().map(window_to_json).collect()
}

pub fn window_from_json(value: &Value) -> Result<Window, String> {
    let object = value.as_object().ok_or("window must be an object")?;
    check_keys(
        object,
        &[
            "id",
            "app",
            "class",
            "title",
            "workspace",
            "monitor",
            "focused",
            "minimized",
            "maximized",
            "floating",
            "fullscreen",
        ],
    )?;
    let window = Window {
        id: object_text(object, "id")?,
        app: object_text(object, "app")?,
        class: object_text(object, "class")?,
        title: object_text(object, "title")?,
        workspace: object_text(object, "workspace")?,
        monitor: object_text(object, "monitor")?,
        focused: object_flag(object, "focused")?,
        minimized: object_flag(object, "minimized")?,
        maximized: object_flag(object, "maximized")?,
        floating: object_flag(object, "floating")?,
        fullscreen: object_flag(object, "fullscreen")?,
    };
    window.validate().map_err(|error| error.to_string())?;
    Ok(window)
}

/// One change as a `watch --json` line.
pub fn change_to_json(change: &Change) -> Value {
    match change {
        Change::Facts {
            revision,
            changed,
            removed,
        } => json!({
            "revision": revision,
            "facts": facts_to_json(changed),
            "removed": removed.iter().map(|key| key.name()).collect::<Vec<_>>(),
        }),
        Change::Windows {
            revision,
            upserted,
            removed,
        } => json!({
            "revision": revision,
            "windows": windows_to_json(upserted),
            "removed": removed,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_round_trip() {
        let input = json!({
            "focus.app": "org.gnome.Ptyxis",
            "session.locked": false,
            "display.brightness": 0.25,
            "workspaces": [{"id": "1", "name": "One", "monitor": "eDP-1", "active": true, "mode": "floating"}],
        });
        let facts = facts_from_json(input.as_object().unwrap()).unwrap();
        assert_eq!(facts_to_json(&facts), input);
    }

    #[test]
    fn strict_parsing() {
        assert!(facts_from_json(json!({"app.id": "x"}).as_object().unwrap()).is_err());
        assert!(facts_from_json(json!({"session.locked": 1}).as_object().unwrap()).is_err());
        assert!(window_from_json(&json!({"id": "1", "app": "a", "x": 1})).is_err());
        assert!(window_from_json(&json!({"id": "1"})).is_err());
    }
}
