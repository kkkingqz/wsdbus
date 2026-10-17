//! The shared cases of `spec/`; every provider must pass them.

use serde_json::Value;
use wsdbus_contract::appid;
use wsdbus_contract::sandbox::{Evidence, Probe, classify};

fn cases(text: &str) -> Vec<Value> {
    let document: Value = serde_json::from_str(text).unwrap();
    document["cases"].as_array().unwrap().clone()
}

#[test]
fn appid_cases() {
    let cases = cases(include_str!("../../../spec/appid-cases.json"));
    assert!(!cases.is_empty());
    for case in cases {
        let app = appid::normalize(case["desktop_id"].as_str(), case["class"].as_str());
        assert_eq!(app, case["app"].as_str().unwrap(), "case {}", case["name"]);
    }
}

fn probe_state(name: &str) -> Probe<()> {
    match name {
        "present" => Probe::Present(()),
        "absent" => Probe::Absent,
        "failed" => Probe::Failed,
        other => panic!("unknown probe {other}"),
    }
}

#[test]
fn sandbox_cases() {
    let cases = cases(include_str!("../../../spec/sandbox-cases.json"));
    assert!(!cases.is_empty());
    for case in cases {
        let cgroup = match probe_state(case["cgroup"]["probe"].as_str().unwrap()) {
            Probe::Present(()) => {
                Probe::Present(case["cgroup"]["text"].as_str().unwrap().to_owned())
            }
            Probe::Absent => Probe::Absent,
            Probe::Failed => Probe::Failed,
        };
        let evidence = Evidence {
            pid: case["pid"].as_bool().unwrap(),
            flatpak_info: probe_state(case["flatpak_info"].as_str().unwrap()),
            cgroup,
        };
        assert_eq!(
            classify(&evidence).name(),
            case["verdict"].as_str().unwrap(),
            "case {}",
            case["name"]
        );
    }
}
