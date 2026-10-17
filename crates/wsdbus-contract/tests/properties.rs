//! Property tests: wire and JSON round trips, and a mirror fed by a store.

use proptest::prelude::*;
use wsdbus_contract::{
    Applied, Change, FactKey, FactValue, Facts, Mirror, Store, Window, Workspace, WorkspaceMode,
    json, wire,
};

fn text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9.:_ -]{1,12}"
}

fn workspace() -> impl Strategy<Value = Workspace> {
    (
        "[0-9]{1,2}",
        text(),
        "[A-Z]{1,3}-[0-9]",
        any::<bool>(),
        prop_oneof![
            Just(None),
            Just(Some(WorkspaceMode::Tiling)),
            Just(Some(WorkspaceMode::Floating))
        ],
    )
        .prop_map(|(id, name, monitor, active, mode)| Workspace {
            id,
            name,
            monitor,
            active,
            mode,
        })
}

fn fact_value(key: FactKey) -> BoxedStrategy<FactValue> {
    match key {
        FactKey::FocusFullscreen | FactKey::SessionLocked => {
            any::<bool>().prop_map(FactValue::Bool).boxed()
        }
        FactKey::DisplayBrightness => (0u32..=1000)
            .prop_map(|level| FactValue::Number(f64::from(level) / 1000.0))
            .boxed(),
        FactKey::AppearanceScheme => prop_oneof![Just("light"), Just("dark")]
            .prop_map(|scheme| FactValue::Text(scheme.into()))
            .boxed(),
        FactKey::AppearanceAccent => "#[0-9a-f]{6}".prop_map(FactValue::Text).boxed(),
        FactKey::Workspaces => proptest::collection::btree_map("[0-9]{1,2}", workspace(), 0..4)
            .prop_map(|records| {
                FactValue::Workspaces(
                    records
                        .into_iter()
                        .map(|(id, workspace)| Workspace { id, ..workspace })
                        .collect(),
                )
            })
            .boxed(),
        _ => text().prop_map(FactValue::Text).boxed(),
    }
}

fn facts() -> impl Strategy<Value = Facts> {
    proptest::sample::subsequence(FactKey::ALL.to_vec(), 0..=FactKey::ALL.len()).prop_flat_map(
        |keys| {
            keys.into_iter()
                .map(|key| fact_value(key).prop_map(move |value| (key, value)))
                .collect::<Vec<_>>()
                .prop_map(|entries| entries.into_iter().collect())
        },
    )
}

fn window() -> impl Strategy<Value = Window> {
    (
        "[0-9]{1,2}",
        text(),
        text(),
        "[ -~]{0,20}",
        proptest::array::uniform5(any::<bool>()),
    )
        .prop_map(|(id, app, class, title, flags)| Window {
            id,
            app,
            class,
            title,
            workspace: "1".into(),
            monitor: "eDP-1".into(),
            focused: flags[0],
            minimized: flags[1],
            maximized: flags[2],
            floating: flags[3],
            fullscreen: flags[4],
        })
}

#[derive(Clone, Debug)]
enum Op {
    Facts(Facts, Vec<FactKey>),
    Windows(Vec<Window>, Vec<String>),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (
            facts(),
            proptest::sample::subsequence(FactKey::ALL.to_vec(), 0..3)
        )
            .prop_map(|(facts, removed)| Op::Facts(facts, removed)),
        (
            proptest::collection::vec(window(), 0..3),
            proptest::collection::vec("[0-9]{1,2}", 0..3)
        )
            .prop_map(|(windows, removed)| Op::Windows(windows, removed)),
    ]
}

fn run(store: &mut Store, op: Op) -> Option<Change> {
    match op {
        Op::Facts(facts, removed) => store.update_facts(facts, &removed).unwrap(),
        Op::Windows(windows, removed) => store.update_windows(windows, &removed).unwrap(),
    }
}

fn same(mirror: &Mirror, store: &Store) -> bool {
    mirror.facts() == store.facts() && mirror.windows().eq(store.windows())
}

proptest! {
    #[test]
    fn facts_wire_round_trip(facts in facts()) {
        let decoded = wire::decode_facts(&wire::encode_facts(&facts));
        prop_assert!(decoded.rejected.is_empty());
        prop_assert_eq!(decoded.value, facts);
    }

    #[test]
    fn windows_wire_round_trip(windows in proptest::collection::vec(window(), 0..5)) {
        let decoded = wire::decode_windows(&wire::encode_windows(&windows));
        prop_assert!(decoded.rejected.is_empty());
        prop_assert_eq!(decoded.value, windows);
    }

    #[test]
    fn facts_json_round_trip(facts in facts()) {
        let encoded = json::facts_to_json(&facts);
        prop_assert_eq!(json::facts_from_json(encoded.as_object().unwrap()).unwrap(), facts);
    }

    #[test]
    fn window_json_round_trip(window in window()) {
        prop_assert_eq!(json::window_from_json(&json::window_to_json(&window)).unwrap(), window);
    }

    /// Every change bumps the revision by exactly one.
    #[test]
    fn revisions_are_consecutive(ops in proptest::collection::vec(op(), 0..30)) {
        let mut store = Store::new();
        let mut last = 0;
        for op in ops {
            if let Some(change) = run(&mut store, op) {
                prop_assert_eq!(change.revision(), last + 1);
                last = change.revision();
            }
            prop_assert_eq!(store.revision(), last);
        }
    }

    /// Facts snapshot at one point, windows snapshot at a later one, then
    /// every change after the facts snapshot: the mirror ends equal to the
    /// store.
    #[test]
    fn split_snapshot_converges(
        ops in proptest::collection::vec(op(), 0..30),
        facts_at in 0usize..30,
        windows_after in 0usize..10,
    ) {
        let mut store = Store::new();
        let facts_at = facts_at.min(ops.len());
        let windows_at = (facts_at + windows_after).min(ops.len());
        let mut changes = Vec::new();
        let mut facts_snapshot = None;
        let mut windows_snapshot = None;
        for (index, op) in ops.into_iter().enumerate() {
            if index == facts_at {
                facts_snapshot = Some((store.revision(), store.facts().clone()));
            }
            if index == windows_at {
                windows_snapshot = Some((store.revision(), store.windows().cloned().collect::<Vec<_>>()));
            }
            changes.extend(run(&mut store, op));
        }
        let (facts_revision, facts) =
            facts_snapshot.unwrap_or_else(|| (store.revision(), store.facts().clone()));
        let (windows_revision, windows) =
            windows_snapshot.unwrap_or_else(|| (store.revision(), store.windows().cloned().collect()));
        let mut mirror = Mirror::new();
        mirror.snapshot(facts_revision, facts, windows_revision, windows);
        for change in changes.iter().filter(|change| change.revision() > facts_revision) {
            let applied = mirror.apply(change);
            prop_assert!(applied == Applied::Changed || applied == Applied::Stale);
        }
        prop_assert!(same(&mirror, &store));
        prop_assert_eq!(mirror.revision(), store.revision());
    }

    /// With changes dropped in transit, the mirror either reports a gap or
    /// equals the store as of the last delivered change.
    #[test]
    fn dropped_changes_are_detected(
        ops in proptest::collection::vec(op(), 1..30),
        drops in proptest::collection::vec(any::<bool>(), 30),
    ) {
        let mut store = Store::new();
        let mut mirror = Mirror::new();
        mirror.snapshot(0, Facts::new(), 0, Vec::new());
        let mut gap = false;
        let mut expected = Store::new();
        for (op, drop) in ops.into_iter().zip(drops) {
            let Some(change) = run(&mut store, op) else { continue };
            if drop {
                continue;
            }
            match mirror.apply(&change) {
                Applied::Changed => expected = store.clone(),
                Applied::Gap => { gap = true; break; }
                other => prop_assert!(false, "unexpected {:?}", other),
            }
        }
        if !gap {
            prop_assert!(same(&mirror, &expected));
        }
    }
}
