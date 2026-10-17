//! D-Bus encoding of facts (`a{sv}`) and windows (`aa{sv}`).
//!
//! Decoding is lenient: unknown fact names and window keys are skipped, and a
//! known fact or a window with a wrong value is dropped and reported in
//! `rejected`, so one bad value does not break a consumer.

use std::collections::BTreeMap;

use zvariant::{Array, Dict, OwnedValue, Signature, Value};

use crate::fact::{self, FactKey, FactValue, Facts, Workspace, WorkspaceMode};
use crate::window::Window;

/// An `a{sv}` dictionary.
pub type WireDict = BTreeMap<String, OwnedValue>;

/// A decoded value and the names of entries that were dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded<T> {
    pub value: T,
    pub rejected: Vec<String>,
}

fn owned(value: Value<'_>) -> OwnedValue {
    value
        .try_into_owned()
        .expect("contract values contain no file descriptors")
}

fn peel<'a>(mut value: &'a Value<'a>) -> &'a Value<'a> {
    while let Value::Value(inner) = value {
        value = inner;
    }
    value
}

fn text<'a>(value: &'a Value<'a>) -> Option<&'a str> {
    match peel(value) {
        Value::Str(text) => Some(text.as_str()),
        _ => None,
    }
}

fn boolean(value: &Value<'_>) -> Option<bool> {
    match peel(value) {
        Value::Bool(flag) => Some(*flag),
        _ => None,
    }
}

fn record_signature() -> Signature {
    Signature::dict(Signature::Str, Signature::Variant)
}

fn record(entries: Vec<(&str, Value<'static>)>) -> Value<'static> {
    let map: BTreeMap<String, Value<'static>> = entries
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
    Value::Dict(Dict::from(map))
}

fn record_entries<'a>(value: &'a Value<'a>) -> Option<BTreeMap<&'a str, &'a Value<'a>>> {
    let Value::Dict(dict) = peel(value) else {
        return None;
    };
    dict.iter()
        .map(|(key, value)| Some((text(key)?, value)))
        .collect()
}

fn encode_value(value: &FactValue) -> OwnedValue {
    match value {
        FactValue::Text(text) => owned(Value::from(text.clone())),
        FactValue::Bool(flag) => owned(Value::from(*flag)),
        FactValue::Number(number) => owned(Value::from(*number)),
        FactValue::Workspaces(records) => {
            let mut array = Array::new(&record_signature());
            for workspace in records {
                let mut entries = vec![
                    ("id", Value::from(workspace.id.clone())),
                    ("name", Value::from(workspace.name.clone())),
                    ("monitor", Value::from(workspace.monitor.clone())),
                    ("active", Value::from(workspace.active)),
                ];
                if let Some(mode) = workspace.mode {
                    entries.push(("mode", Value::from(mode.name())));
                }
                array
                    .append(record(entries))
                    .expect("workspace records share one signature");
            }
            owned(Value::Array(array))
        }
    }
}

fn decode_workspace(value: &Value<'_>) -> Option<Workspace> {
    let entries = record_entries(value)?;
    let field = |name: &str| match entries.get(name) {
        Some(value) => text(value).map(str::to_owned),
        None => Some(String::new()),
    };
    let mode = match entries.get("mode") {
        Some(value) => Some(WorkspaceMode::from_name(text(value)?)?),
        None => None,
    };
    Some(Workspace {
        id: field("id")?,
        name: field("name")?,
        monitor: field("monitor")?,
        active: match entries.get("active") {
            Some(value) => boolean(value)?,
            None => false,
        },
        mode,
    })
}

fn decode_value(key: FactKey, value: &Value<'_>) -> Option<FactValue> {
    let value = match (key.kind(), peel(value)) {
        (fact::ValueKind::Text, Value::Str(text)) => FactValue::Text(text.as_str().to_owned()),
        (fact::ValueKind::Bool, Value::Bool(flag)) => FactValue::Bool(*flag),
        (fact::ValueKind::Number, Value::F64(number)) => FactValue::Number(*number),
        (fact::ValueKind::Workspaces, Value::Array(array)) => FactValue::Workspaces(
            array
                .iter()
                .map(decode_workspace)
                .collect::<Option<Vec<_>>>()?,
        ),
        _ => return None,
    };
    fact::validate(key, &value).ok()?;
    Some(value)
}

pub fn encode_facts(facts: &Facts) -> WireDict {
    facts
        .iter()
        .map(|(key, value)| (key.name().to_owned(), encode_value(value)))
        .collect()
}

pub fn decode_facts(dict: &WireDict) -> Decoded<Facts> {
    let mut decoded = Decoded {
        value: Facts::new(),
        rejected: Vec::new(),
    };
    for (name, value) in dict {
        let Some(key) = FactKey::from_name(name) else {
            continue;
        };
        match decode_value(key, value) {
            Some(value) => {
                decoded.value.insert(key, value);
            }
            None => decoded.rejected.push(name.clone()),
        }
    }
    decoded
}

pub fn encode_fact_names(keys: &[FactKey]) -> Vec<String> {
    keys.iter().map(|key| key.name().to_owned()).collect()
}

/// Known fact names; unknown names are skipped.
pub fn decode_fact_names(names: &[String]) -> Vec<FactKey> {
    names
        .iter()
        .filter_map(|name| FactKey::from_name(name))
        .collect()
}

pub fn encode_window(window: &Window) -> WireDict {
    let text = |value: &String| owned(Value::from(value.clone()));
    let flag = |value: bool| owned(Value::from(value));
    WireDict::from([
        ("id".into(), text(&window.id)),
        ("app".into(), text(&window.app)),
        ("class".into(), text(&window.class)),
        ("title".into(), text(&window.title)),
        ("workspace".into(), text(&window.workspace)),
        ("monitor".into(), text(&window.monitor)),
        ("focused".into(), flag(window.focused)),
        ("minimized".into(), flag(window.minimized)),
        ("maximized".into(), flag(window.maximized)),
        ("floating".into(), flag(window.floating)),
        ("fullscreen".into(), flag(window.fullscreen)),
    ])
}

/// Decodes one window; `id` and `app` are required, other keys default to
/// empty text and `false`.
pub fn decode_window(dict: &WireDict) -> Option<Window> {
    let field = |name: &str| match dict.get(name) {
        Some(value) => text(value).map(str::to_owned),
        None => Some(String::new()),
    };
    let flag = |name: &str| match dict.get(name) {
        Some(value) => boolean(value),
        None => Some(false),
    };
    let window = Window {
        id: field("id")?,
        app: field("app")?,
        class: field("class")?,
        title: field("title")?,
        workspace: field("workspace")?,
        monitor: field("monitor")?,
        focused: flag("focused")?,
        minimized: flag("minimized")?,
        maximized: flag("maximized")?,
        floating: flag("floating")?,
        fullscreen: flag("fullscreen")?,
    };
    window.validate().ok()?;
    Some(window)
}

pub fn encode_windows(windows: &[Window]) -> Vec<WireDict> {
    windows.iter().map(encode_window).collect()
}

/// Decodes windows; a dropped window is reported by its id, or by its index
/// when it has no readable id.
pub fn decode_windows(dicts: &[WireDict]) -> Decoded<Vec<Window>> {
    let mut decoded = Decoded {
        value: Vec::new(),
        rejected: Vec::new(),
    };
    for (index, dict) in dicts.iter().enumerate() {
        match decode_window(dict) {
            Some(window) => decoded.value.push(window),
            None => decoded.rejected.push(
                dict.get("id")
                    .and_then(|value| text(value))
                    .map_or_else(|| format!("#{index}"), str::to_owned),
            ),
        }
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_invalid_facts() {
        let dict = WireDict::from([
            ("future.fact".into(), owned(Value::from(1u32))),
            ("session.locked".into(), owned(Value::from("yes"))),
            ("appearance.scheme".into(), owned(Value::from("sepia"))),
            ("focus.app".into(), owned(Value::from("org.gnome.Nautilus"))),
        ]);
        let decoded = decode_facts(&dict);
        assert_eq!(
            decoded.value,
            Facts::from([(
                FactKey::FocusApp,
                FactValue::Text("org.gnome.Nautilus".into())
            )])
        );
        assert_eq!(decoded.rejected, ["appearance.scheme", "session.locked"]);
    }

    #[test]
    fn window_defaults_and_required_keys() {
        let dict = WireDict::from([
            ("id".into(), owned(Value::from("7"))),
            ("app".into(), owned(Value::from("wmclass:xterm"))),
            ("extra".into(), owned(Value::from(3u32))),
        ]);
        let window = decode_window(&dict).unwrap();
        assert_eq!(window.id, "7");
        assert!(!window.focused);
        let missing_app = WireDict::from([("id".into(), owned(Value::from("8")))]);
        let decoded = decode_windows(&[dict, missing_app]);
        assert_eq!(decoded.value.len(), 1);
        assert_eq!(decoded.rejected, ["8"]);
    }
}
